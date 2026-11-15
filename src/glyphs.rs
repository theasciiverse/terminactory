//! Item glyphs: the one character an item shows on belts and in the book.
//!
//! The built-in glyphs are plain ASCII (in `ui::item_cell`). This module can replace them:
//! with the "Icons" setting on Nerd Font, items that have a fitting icon use it (the rest keep
//! their ASCII glyph), and glyphs.json can replace any item's glyph with any one-cell character,
//! with or without Nerd Fonts. glyphs.json wins over the icons.

use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use crate::content::Item;

static TABLE: RwLock<Option<HashMap<Item, char>>> = RwLock::new(None);

/// The glyph that replaces `item`'s built-in one, if any.
pub fn replacement(item: Item) -> Option<char> {
    TABLE.read().ok()?.as_ref()?.get(&item).copied()
}

/// Builds the table for the current setting and glyphs.json (when it exists) and makes it the one
/// in use. Returns a note for the player when the file has something wrong in it.
pub fn apply(nerd: bool, file: &Path) -> Option<String> {
    let (table, note) = build(nerd, std::fs::read_to_string(file).ok().as_deref());
    if let Ok(mut t) = TABLE.write() {
        *t = (!table.is_empty()).then_some(table);
    }
    note
}

/// The table for `nerd` and the text of glyphs.json. Entries that can't be used are skipped and
/// named in the note.
fn build(nerd: bool, file: Option<&str>) -> (HashMap<Item, char>, Option<String>) {
    let mut table: HashMap<Item, char> = if nerd { NERD.iter().copied().collect() } else { HashMap::new() };
    let Some(text) = file else { return (table, None) };
    let map: HashMap<String, String> = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => return (table, Some(format!("glyphs.json has a mistake ({e}); ignoring it"))),
    };
    let mut bad = Vec::new();
    for (name, glyph) in map {
        let key = |s: &str| s.replace(['_', ' ', '-'], "").to_ascii_lowercase();
        let item = Item::ALL.iter().copied().find(|i| key(i.name()) == key(&name));
        let mut chars = glyph.chars();
        match (item, chars.next(), chars.next()) {
            (Some(item), Some(c), None) if one_cell(c) => {
                table.insert(item, c);
            }
            _ => bad.push(name),
        }
    }
    bad.sort();
    let note =
        (!bad.is_empty()).then(|| format!("glyphs.json: can't use {} (needs an item name and one single-width character)", bad.join(", ")));
    (table, note)
}

/// Whether the terminal gives `c` exactly one cell, which the map layout relies on.
fn one_cell(c: char) -> bool {
    ratatui::text::Span::raw(c.to_string()).width() == 1
}

/// Nerd Font icons (Font Awesome set, in the private-use area) for items that have a good one.
const NERD: &[(Item, char)] = &[
    (Item::Coal, '\u{f06d}'),           // fire
    (Item::Waste, '\u{f1f8}'),          // trash
    (Item::IronGear, '\u{f013}'),       // cog
    (Item::HardenedGear, '\u{f013}'),   // cog
    (Item::Gearbox, '\u{f085}'),        // cogs
    (Item::Glass, '\u{f000}'),          // glass
    (Item::Valve, '\u{f0ad}'),          // wrench
    (Item::Gauge, '\u{f0e4}'),          // gauge
    (Item::Lamp, '\u{f0eb}'),           // light bulb
    (Item::LampMatrix, '\u{f0eb}'),     // light bulb
    (Item::Relay, '\u{f0e7}'),          // bolt
    (Item::Generator, '\u{f0e7}'),      // bolt
    (Item::Transformer, '\u{f0e7}'),    // bolt
    (Item::PumpUnit, '\u{f043}'),       // drop
    (Item::SteamTractor, '\u{f0d1}'),   // truck
    (Item::Tube, '\u{f0c3}'),           // flask
    (Item::Etchant, '\u{f0c3}'),        // flask
    (Item::Terminal, '\u{f120}'),       // terminal prompt
    (Item::Microprocessor, '\u{f2db}'), // chip
    (Item::FineMicroprocessor, '\u{f2db}'),
    (Item::RamChip, '\u{f2db}'),
    (Item::Mainboard, '\u{f2db}'),
    (Item::Keyboard, '\u{f11c}'),  // keyboard
    (Item::DiskDrive, '\u{f0a0}'), // disk
    (Item::Computer, '\u{f108}'),  // desktop
    (Item::Brick, '\u{f0c8}'),     // square
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nerd_icons_are_one_cell_and_unique_per_item() {
        for (item, c) in NERD {
            assert!(one_cell(*c), "{item:?}");
        }
        let items: std::collections::HashSet<_> = NERD.iter().map(|(i, _)| *i).collect();
        assert_eq!(items.len(), NERD.len(), "an item is listed twice");
    }

    #[test]
    fn icons_are_off_unless_asked_for() {
        assert!(build(false, None).0.is_empty());
        assert_eq!(build(true, None).0.get(&Item::Coal), Some(&'\u{f06d}'));
    }

    #[test]
    fn glyphs_json_wins_and_names_are_forgiving() {
        let (t, note) = build(true, Some(r#"{"coal": "C", "Iron_Gear": "G", "CopperOre": "◆"}"#));
        assert!(note.is_none(), "{note:?}");
        assert_eq!((t[&Item::Coal], t[&Item::IronGear], t[&Item::CopperOre]), ('C', 'G', '◆'));
        // Untouched icons stay.
        assert_eq!(t[&Item::Lamp], '\u{f0eb}');
    }

    #[test]
    fn bad_entries_are_skipped_and_named() {
        let (t, note) = build(false, Some(r#"{"coal": "ab", "nothing": "x", "stone": "日", "sand": ""}"#));
        assert!(t.is_empty());
        let note = note.unwrap();
        for name in ["coal", "nothing", "stone", "sand"] {
            assert!(note.contains(name), "{note}");
        }
    }

    #[test]
    fn a_broken_file_is_ignored_with_a_note() {
        let (t, note) = build(true, Some("{ not json"));
        assert!(!t.is_empty());
        assert!(note.unwrap().contains("glyphs.json"));
    }
}
