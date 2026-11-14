//! Game settings, kept in settings.json (global, not per save). Like controls.json it is created
//! with the defaults when missing, and a bad file never stops the game: every field that can't be
//! read falls back to its default and the player is told.
//!
//! To add a setting: a field on `Settings`, a `Choice` enum if it picks from a list, a line in
//! `Settings::load_or_create`'s field list, and one entry in `ROWS` (that is what the pane shows;
//! its `enabled` says when the setting has an effect, and the pane greys the row out when it has none).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::history::Range;

/// A setting that cycles through a fixed list of named values.
pub trait Choice: Copy + PartialEq + 'static {
    const ALL: &'static [Self];
    fn name(self) -> &'static str;
    /// The next value (`step` 1) or the previous one (-1), wrapping round.
    fn step(self, step: i32) -> Self {
        let n = Self::ALL.len() as i32;
        let i = Self::ALL.iter().position(|c| *c == self).unwrap_or(0) as i32;
        Self::ALL[(i + step).rem_euclid(n) as usize]
    }
}

/// Which charts the sidebar shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphShow {
    Off,
    #[default]
    Money,
    Research,
    Both,
}

impl Choice for GraphShow {
    const ALL: &'static [Self] = &[GraphShow::Off, GraphShow::Money, GraphShow::Research, GraphShow::Both];
    fn name(self) -> &'static str {
        match self {
            GraphShow::Off => "Off",
            GraphShow::Money => "Money",
            GraphShow::Research => "Research",
            GraphShow::Both => "Both",
        }
    }
}

/// When the compact small-pane view is used.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmallView {
    /// Below the cut-off width, or a short pane.
    #[default]
    Auto,
    Always,
    Never,
}

impl Choice for SmallView {
    const ALL: &'static [Self] = &[SmallView::Auto, SmallView::Always, SmallView::Never];
    fn name(self) -> &'static str {
        match self {
            SmallView::Auto => "Auto",
            SmallView::Always => "Always",
            SmallView::Never => "Never",
        }
    }
}

/// The width in columns below which Auto switches to the small-pane view. Only these values
/// are offered, and settings.json accepts only these.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(transparent)]
pub struct Cols(pub u16);

impl Default for Cols {
    fn default() -> Self {
        Cols(crate::ui::SMALL_W)
    }
}

impl<'de> Deserialize<'de> for Cols {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let n = u16::deserialize(d)?;
        Cols::ALL.iter().copied().find(|c| c.0 == n).ok_or_else(|| serde::de::Error::custom("not one of 80, 100, 120, 140, 160"))
    }
}

impl Choice for Cols {
    const ALL: &'static [Self] = &[Cols(80), Cols(100), Cols(120), Cols(140), Cols(160)];
    fn name(self) -> &'static str {
        match self.0 {
            80 => "80",
            100 => "100",
            120 => "120",
            140 => "140",
            _ => "160",
        }
    }
}

/// Where a waiting telegram shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TelegramStyle {
    /// A box at the bottom of the sidebar (a corner note in the small pane).
    #[default]
    Sidebar,
    /// Only the short "agent finished · n to view" note, in both views.
    Note,
    /// Only the mail count in the top line.
    Ticker,
}

impl Choice for TelegramStyle {
    const ALL: &'static [Self] = &[TelegramStyle::Sidebar, TelegramStyle::Note, TelegramStyle::Ticker];
    fn name(self) -> &'static str {
        match self {
            TelegramStyle::Sidebar => "Sidebar",
            TelegramStyle::Note => "Note",
            TelegramStyle::Ticker => "Ticker",
        }
    }
}

/// A plain on/off switch (on by default).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnOff {
    #[default]
    On,
    Off,
}

impl Choice for OnOff {
    const ALL: &'static [Self] = &[OnOff::On, OnOff::Off];
    fn name(self) -> &'static str {
        match self {
            OnOff::On => "On",
            OnOff::Off => "Off",
        }
    }
}

/// How much time each bar of the sidebar chart covers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphRange {
    Seconds,
    #[default]
    Minutes,
    Hours,
}

impl Choice for GraphRange {
    const ALL: &'static [Self] = &[GraphRange::Seconds, GraphRange::Minutes, GraphRange::Hours];
    fn name(self) -> &'static str {
        match self {
            GraphRange::Seconds => "Seconds",
            GraphRange::Minutes => "Minutes",
            GraphRange::Hours => "Hours",
        }
    }
}

impl GraphRange {
    pub fn range(self) -> Range {
        match self {
            GraphRange::Seconds => Range::Seconds,
            GraphRange::Minutes => Range::Minutes,
            GraphRange::Hours => Range::Hours,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub sidebar_graph: GraphShow,
    pub sidebar_graph_range: GraphRange,
    pub small_view: SmallView,
    pub small_width: Cols,
    pub telegram_style: TelegramStyle,
    /// Quiet hints and asides on the idle status line.
    pub hints: OnOff,
    /// T Web bookmarks: wiki page slugs, in the order the bar shows them.
    pub bookmarks: Vec<String>,
    /// T Web searches, newest first.
    pub search_history: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            sidebar_graph: GraphShow::default(),
            sidebar_graph_range: GraphRange::default(),
            small_view: SmallView::default(),
            small_width: Cols::default(),
            telegram_style: TelegramStyle::default(),
            hints: OnOff::default(),
            bookmarks: DEFAULT_BOOKMARKS.iter().map(|s| s.to_string()).collect(),
            search_history: Vec::new(),
        }
    }
}

/// The bookmarks a new settings file starts with.
pub const DEFAULT_BOOKMARKS: &[&str] = &["home", "getting-started", "machines", "items"];

impl Settings {
    /// Whether a pane of this size gets the small-pane view (before `m`'s session override).
    pub fn small_pane(&self, width: u16, height: u16) -> bool {
        match self.small_view {
            SmallView::Always => true,
            // Never stays in the full view, except where that view has no room to draw at all.
            SmallView::Never => width < crate::ui::FULL_MIN_W || height < crate::ui::FULL_MIN_H,
            SmallView::Auto => width < self.small_width.0 || height < crate::ui::SMALL_H,
        }
    }
}

/// One line of the settings pane.
pub struct Row {
    pub name: &'static str,
    /// What the row does, for the pane's hint line.
    pub help: &'static str,
    /// The current value's name.
    pub value: fn(&Settings) -> &'static str,
    /// Moves the value on (1) or back (-1).
    pub change: fn(&mut Settings, i32),
    /// False when the setting has no effect with the others as they are (the pane greys it out).
    pub enabled: fn(&Settings) -> bool,
}

/// The rows of the settings pane, top to bottom.
pub const ROWS: &[Row] = &[
    Row {
        name: "Sidebar graph",
        help: "Earnings charts under the build menu (full view only)",
        value: |s| s.sidebar_graph.name(),
        change: |s, d| s.sidebar_graph = s.sidebar_graph.step(d),
        enabled: |_| true,
    },
    Row {
        name: "Graph range",
        help: "What each bar of the sidebar graph covers",
        value: |s| s.sidebar_graph_range.name(),
        change: |s, d| s.sidebar_graph_range = s.sidebar_graph_range.step(d),
        enabled: |_| true,
    },
    Row {
        name: "Small-pane view",
        help: "Compact view: Auto, Always or Never (m toggles it for now)",
        value: |s| s.small_view.name(),
        change: |s, d| s.small_view = s.small_view.step(d),
        enabled: |_| true,
    },
    Row {
        name: "Small-pane width",
        help: "Auto switches below this many columns (Auto only)",
        value: |s| s.small_width.name(),
        change: |s, d| s.small_width = s.small_width.step(d),
        enabled: |s| s.small_view == SmallView::Auto,
    },
    Row {
        name: "Telegram style",
        help: "Sidebar box, a short Note, or just a mail count (Ticker)",
        value: |s| s.telegram_style.name(),
        change: |s, d| s.telegram_style = s.telegram_style.step(d),
        enabled: |_| true,
    },
    Row {
        name: "Hints",
        help: "Quiet diagnoses and asides on the idle status line",
        value: |s| s.hints.name(),
        change: |s, d| s.hints = s.hints.step(d),
        enabled: |_| true,
    },
];

impl Settings {
    /// Loads settings.json, creating it with the defaults if it doesn't exist. Returns a note for
    /// the player when something needs their attention.
    pub fn load_or_create(path: &Path) -> (Self, Option<String>) {
        let Ok(text) = std::fs::read_to_string(path) else {
            let s = Settings::default();
            let note = s.save(path).err().map(|e| format!("Couldn't write settings.json ({e}); using default settings"));
            return (s, note);
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => return (Self::default(), Some(format!("settings.json has a mistake ({e}); using default settings"))),
        };
        let Some(map) = value.as_object() else {
            return (
                Self::default(),
                Some("settings.json should be an object like {\"sidebar_graph\": \"money\"}; using default settings".into()),
            );
        };
        // Each field on its own: one bad value costs only that setting. Unknown fields are ignored.
        let mut s = Settings::default();
        let mut problems = Vec::new();
        fn field<T: serde::de::DeserializeOwned + Default>(
            map: &serde_json::Map<String, serde_json::Value>,
            name: &str,
            problems: &mut Vec<String>,
        ) -> T {
            match map.get(name) {
                None => T::default(),
                Some(v) => serde_json::from_value(v.clone()).unwrap_or_else(|_| {
                    problems.push(format!("\"{name}\" is {v}"));
                    T::default()
                }),
            }
        }
        s.sidebar_graph = field(map, "sidebar_graph", &mut problems);
        s.sidebar_graph_range = field(map, "sidebar_graph_range", &mut problems);
        s.small_view = field(map, "small_view", &mut problems);
        s.small_width = field(map, "small_width", &mut problems);
        s.telegram_style = field(map, "telegram_style", &mut problems);
        s.hints = field(map, "hints", &mut problems);
        if map.contains_key("bookmarks") {
            s.bookmarks = field(map, "bookmarks", &mut problems);
        }
        if map.contains_key("search_history") {
            s.search_history = field(map, "search_history", &mut problems);
        }
        let note = (!problems.is_empty()).then(|| format!("settings.json: {}; using the default there", problems.join(", ")));
        (s, note)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text + "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("terminactory-settings-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn defaults_are_money_by_minute() {
        let s = Settings::default();
        assert_eq!((s.sidebar_graph, s.sidebar_graph_range), (GraphShow::Money, GraphRange::Minutes));
    }

    #[test]
    fn missing_file_is_created_with_defaults() {
        let path = temp("new");
        std::fs::remove_file(&path).ok();
        let (s, note) = Settings::load_or_create(&path);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert!(note.is_none());
        assert_eq!(s, Settings::default());
        assert!(text.contains("\"sidebar_graph\": \"money\"") && text.contains("\"sidebar_graph_range\": \"minutes\""), "{text}");
    }

    #[test]
    fn settings_round_trip() {
        let path = temp("round");
        let s = Settings {
            sidebar_graph: GraphShow::Both,
            sidebar_graph_range: GraphRange::Hours,
            small_view: SmallView::Never,
            small_width: Cols(140),
            telegram_style: TelegramStyle::Ticker,
            hints: OnOff::Off,
            bookmarks: vec!["power".to_string(), "waste".to_string()],
            search_history: vec!["coal".to_string(), "belt".to_string()],
        };
        s.save(&path).unwrap();
        let (back, note) = Settings::load_or_create(&path);
        std::fs::remove_file(&path).ok();
        assert!(note.is_none());
        assert_eq!(back, s);
    }

    #[test]
    fn unknown_and_missing_fields_are_fine() {
        let path = temp("fields");
        std::fs::write(&path, r#"{ "sidebar_graph": "research", "something_new": [1, 2], "_note": "hi" }"#).unwrap();
        let (s, note) = Settings::load_or_create(&path);
        std::fs::remove_file(&path).ok();
        assert!(note.is_none(), "{note:?}");
        assert_eq!(s.sidebar_graph, GraphShow::Research);
        assert_eq!(s.sidebar_graph_range, GraphRange::Minutes); // missing: the default
    }

    #[test]
    fn the_new_settings_default_to_todays_behaviour() {
        let s = Settings::default();
        assert_eq!(
            (s.small_view, s.small_width, s.telegram_style, s.hints),
            (SmallView::Auto, Cols(120), TelegramStyle::Sidebar, OnOff::On)
        );
        assert_eq!(s.small_width.0, crate::ui::SMALL_W);
    }

    #[test]
    fn the_new_fields_are_written_by_name() {
        let path = temp("names");
        let s = Settings {
            small_view: SmallView::Always,
            small_width: Cols(80),
            telegram_style: TelegramStyle::Note,
            hints: OnOff::Off,
            ..Default::default()
        };
        s.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        for want in [r#""small_view": "always""#, r#""small_width": 80"#, r#""telegram_style": "note""#, r#""hints": "off""#] {
            assert!(text.contains(want), "{want}: {text}");
        }
    }

    #[test]
    fn an_old_file_without_the_new_fields_still_loads() {
        let path = temp("old");
        // What a settings.json from before these settings looks like.
        std::fs::write(&path, r#"{ "sidebar_graph": "both", "sidebar_graph_range": "hours" }"#).unwrap();
        let (s, note) = Settings::load_or_create(&path);
        std::fs::remove_file(&path).ok();
        assert!(note.is_none(), "{note:?}");
        assert_eq!((s.sidebar_graph, s.sidebar_graph_range), (GraphShow::Both, GraphRange::Hours));
        assert_eq!(
            (s.small_view, s.small_width, s.telegram_style, s.hints),
            (SmallView::Auto, Cols(120), TelegramStyle::Sidebar, OnOff::On)
        );
    }

    #[test]
    fn a_bad_width_falls_back_alone() {
        let path = temp("width");
        std::fs::write(&path, r#"{ "small_width": 95, "small_view": "never", "hints": "maybe" }"#).unwrap();
        let (s, note) = Settings::load_or_create(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!((s.small_width, s.small_view, s.hints), (Cols(120), SmallView::Never, OnOff::On));
        let note = note.unwrap();
        assert!(note.contains("small_width") && note.contains("hints") && !note.contains("small_view"), "{note}");
    }

    #[test]
    fn the_cut_off_decides_the_small_pane_by_view() {
        let mut s = Settings::default();
        // Auto: below the cut-off width, or a short pane.
        assert!(s.small_pane(119, 40) && !s.small_pane(120, 30) && s.small_pane(200, 29));
        s.small_width = Cols(80);
        assert!(!s.small_pane(80, 30) && s.small_pane(79, 30));
        s.small_width = Cols(160);
        assert!(s.small_pane(159, 50) && !s.small_pane(160, 50));
        // Always and Never ignore the width and height (Never still needs room to draw).
        s.small_view = SmallView::Always;
        assert!(s.small_pane(300, 90));
        s.small_view = SmallView::Never;
        assert!(!s.small_pane(60, 20) && !s.small_pane(46, 12));
        assert!(s.small_pane(45, 40) && s.small_pane(100, 11));
    }

    #[test]
    fn the_width_row_is_only_live_in_auto() {
        let mut s = Settings::default();
        assert!((ROWS[3].enabled)(&s));
        (ROWS[2].change)(&mut s, 1);
        assert_eq!(s.small_view, SmallView::Always);
        assert!(!(ROWS[3].enabled)(&s));
        (ROWS[3].change)(&mut s, 1);
        assert_eq!(s.small_width, Cols(140)); // still changeable, it just has no effect now
        assert!(ROWS[3].help.contains("Auto only"));
    }

    #[test]
    fn a_bad_file_gives_defaults_and_a_note() {
        let path = temp("bad");
        for text in ["{ nope", "[1, 2]", "", "42"] {
            std::fs::write(&path, text).unwrap();
            let (s, note) = Settings::load_or_create(&path);
            assert_eq!(s, Settings::default(), "{text}");
            assert!(note.unwrap().contains("settings.json"), "{text}");
        }
        // One bad value costs only that setting.
        std::fs::write(&path, r#"{ "sidebar_graph": "loud", "sidebar_graph_range": "hours" }"#).unwrap();
        let (s, note) = Settings::load_or_create(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!((s.sidebar_graph, s.sidebar_graph_range), (GraphShow::Money, GraphRange::Hours));
        assert!(note.unwrap().contains("sidebar_graph"));
    }

    #[test]
    fn rows_cycle_both_ways() {
        let mut s = Settings::default();
        (ROWS[0].change)(&mut s, 1);
        assert_eq!(s.sidebar_graph, GraphShow::Research);
        (ROWS[0].change)(&mut s, -1);
        (ROWS[0].change)(&mut s, -1);
        (ROWS[0].change)(&mut s, -1);
        assert_eq!(s.sidebar_graph, GraphShow::Both); // Money -> Research -> Money -> Off -> Both (wraps)
        assert_eq!((ROWS[1].value)(&s), "Minutes");
    }
}
