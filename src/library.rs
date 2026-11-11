//! The blueprint library: one JSON file per blueprint in `~/.terminactory/blueprints`, shared by
//! every save. Each file carries its own `format` number, apart from the save's version.
//!
//! A file that can't be used is listed, never touched: a newer format is "from a newer game", and
//! anything that doesn't parse (an unknown part, a broken box, not JSON at all) is "can't be read"
//! with the reason. Nothing here deletes or overwrites a file it did not make.

use std::io;
use std::path::{Path, PathBuf};

use crate::sim::{Blueprint, BP_FORMAT};

/// Files bigger than this are not read (a whole map is a few hundred kilobytes).
const MAX_FILE: u64 = 8 * 1024 * 1024;

/// Where the blueprints live inside the data folder.
pub fn dir(data: &Path) -> PathBuf {
    data.join("blueprints")
}

/// What a file turned out to hold.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Ok(Blueprint),
    /// Written by a newer game: its format number.
    Newer(u32),
    /// Couldn't be used, and why.
    Broken(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    /// The blueprint's own name, or the file's name when it can't be read.
    pub name: String,
    pub content: Content,
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Reads one file. Never fails: a file that can't be used comes back as `Broken` or `Newer`.
pub fn read(path: &Path) -> Entry {
    let fallback = stem(path);
    let entry = |name: String, content| Entry { path: path.to_path_buf(), name, content };
    let broken = |why: String| entry(fallback.clone(), Content::Broken(why));
    match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_FILE => return broken("the file is far too big".into()),
        Err(e) => return broken(format!("couldn't open it ({e})")),
        _ => {}
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return broken(format!("couldn't read it ({e})")),
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return broken(format!("not JSON ({e})")),
    };
    let name = value.get("name").and_then(|n| n.as_str()).map(str::trim).filter(|n| !n.is_empty()).map_or(fallback.clone(), str::to_string);
    let Some(format) = value.get("format").and_then(|f| f.as_u64()) else {
        return broken("it has no format number".into());
    };
    if format > BP_FORMAT as u64 {
        return entry(name, Content::Newer(format.min(u32::MAX as u64) as u32));
    }
    if format == 0 {
        return broken("its format number is 0".into());
    }
    match serde_json::from_value::<Blueprint>(value) {
        Ok(bp) => match bp.check() {
            Ok(()) => entry(name, Content::Ok(bp)),
            Err(why) => entry(name, Content::Broken(why)),
        },
        Err(e) => entry(name, Content::Broken(e.to_string())),
    }
}

/// Every `.json` file in the library folder, by name (case doesn't matter). A missing folder is
/// an empty library.
pub fn list(data: &Path) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir(data)) else { return Vec::new() };
    let mut out: Vec<Entry> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json") && !stem(p).starts_with('.'))
        .map(|p| read(&p))
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.path.cmp(&b.path)));
    out
}

/// A file name for a blueprint's name: lower-case letters and digits, joined by single dashes.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.trim_end_matches('-').chars().take(40).collect();
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "blueprint".into()
    } else {
        out
    }
}

/// Saves a blueprint as a new file named after it (`-2`, `-3` when the name is taken): written to
/// a temporary file first and then renamed, so a crash never leaves half a file. Returns the path.
pub fn save(data: &Path, bp: &Blueprint) -> io::Result<PathBuf> {
    let folder = dir(data);
    std::fs::create_dir_all(&folder)?;
    let base = slug(&bp.name);
    let mut path = folder.join(format!("{base}.json"));
    let mut n = 2;
    while path.exists() {
        path = folder.join(format!("{base}-{n}.json"));
        n += 1;
    }
    let mut out = bp.clone();
    out.format = BP_FORMAT;
    let text = serde_json::to_string_pretty(&out).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text + "\n")?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(path)
}

/// Removes a library file. Only `.json` files are touched.
pub fn delete(path: &Path) -> io::Result<()> {
    if path.extension().is_none_or(|x| x != "json") {
        return Err(io::Error::other("not a blueprint file"));
    }
    std::fs::remove_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{Dir, Kind, World, H, W};

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("factory-library-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample(name: &str) -> Blueprint {
        let mut w = World::blank();
        w.place(0, 10, 10, Kind::Furnace, Dir::S).unwrap();
        w.place(0, 14, 10, Kind::Belt, Dir::E).unwrap();
        w.place(0, 15, 10, Kind::Tunnel, Dir::E).unwrap();
        w.place(0, 18, 10, Kind::Tunnel, Dir::E).unwrap();
        w.place_sized(0, 10, 15, Kind::Elevator, Dir::E, 3).unwrap();
        let mut bp = w.capture(0, (0, 0, W - 1, H - 1)).bp;
        bp.name = name.to_string();
        bp
    }

    fn write(data: &Path, file: &str, text: &str) -> PathBuf {
        let d = dir(data);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(file);
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn a_saved_blueprint_reads_back_the_same_and_is_listed() {
        let data = temp("round-trip");
        let bp = sample("Copper smelter row");
        assert!(list(&data).is_empty(), "no folder yet is an empty library");
        let path = save(&data, &bp).unwrap();
        assert_eq!(path, dir(&data).join("copper-smelter-row.json"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"format\": 1") && text.contains("\"name\": \"Copper smelter row\""), "{text}");
        let entries = list(&data);
        assert_eq!(entries.len(), 1);
        assert_eq!((entries[0].name.as_str(), &entries[0].content), ("Copper smelter row", &Content::Ok(bp)));
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn saving_leaves_no_temporary_file_and_names_that_collide_get_a_suffix() {
        let data = temp("collide");
        let bp = sample("Row");
        let a = save(&data, &bp).unwrap();
        let b = save(&data, &bp).unwrap();
        let c = save(&data, &sample("ROW!!")).unwrap();
        assert_eq!([a, b, c].map(|p| p.file_name().unwrap().to_string_lossy().into_owned()), ["row.json", "row-2.json", "row-3.json"]);
        let files: Vec<String> =
            std::fs::read_dir(dir(&data)).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(files.len(), 3, "{files:?}");
        assert!(files.iter().all(|f| f.ends_with(".json")), "no temp file left: {files:?}");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn slugs_are_plain_lowercase_words() {
        assert_eq!(slug("Copper smelter row"), "copper-smelter-row");
        assert_eq!(slug("  A/B  c__d  "), "a-b-c-d");
        assert_eq!(slug("Ünï…"), "n");
        assert_eq!(slug("!!!"), "blueprint");
        assert_eq!(slug(""), "blueprint");
        assert!(slug(&"long name ".repeat(20)).len() <= 40);
        assert!(!slug(&"abcd efgh ".repeat(20)).ends_with('-'));
        // Nothing in a name can climb out of the folder.
        assert_eq!(slug("../../etc/passwd"), "etc-passwd");
    }

    #[test]
    fn the_list_is_sorted_by_name_without_caring_about_case() {
        let data = temp("sorted");
        for n in ["banana", "Cherry", "apple", "Apple pie"] {
            save(&data, &sample(n)).unwrap();
        }
        let names: Vec<String> = list(&data).into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["apple", "Apple pie", "banana", "Cherry"]);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn bad_files_are_listed_with_a_reason_and_never_crash_or_vanish() {
        let data = temp("bad");
        let good = sample("Good");
        save(&data, &good).unwrap();
        let ok = serde_json::to_string(&good).unwrap();
        let newer = write(&data, "future.json", r#"{"format": 7, "name": "Hyperloop", "w": 1, "h": 1, "parts": [{"kind": "Teleporter"}]}"#);
        let unknown = write(&data, "unknown-kind.json", &ok.replace("Furnace", "Teleporter"));
        let junk = write(&data, "junk.json", "this is not json {");
        let empty = write(&data, "empty.json", "");
        let no_format = write(&data, "no-format.json", r#"{"name": "x", "w": 1, "h": 1, "parts": []}"#);
        let outside = write(&data, "outside.json", &ok.replace("\"w\":", "\"w\":2,\"was\":"));
        let overlap = write(
            &data,
            "overlap.json",
            r#"{"format":1,"name":"Overlap","w":2,"h":1,"parts":[{"dx":0,"dy":0,"kind":"Belt","dir":"E"},{"dx":0,"dy":0,"kind":"Belt","dir":"N"}]}"#,
        );
        let huge =
            write(&data, "huge.json", &format!(r#"{{"format":1,"name":"Huge","w":9999,"h":9999,"parts":[],"pad":"{}"}}"#, "x".repeat(100)));
        let not_a_file = dir(&data).join("folder.json");
        std::fs::create_dir_all(&not_a_file).unwrap();
        std::fs::write(dir(&data).join("notes.txt"), "hello").unwrap();
        std::fs::write(dir(&data).join(".hidden.json"), "{}").unwrap();
        let entries = list(&data);
        let by = |p: &Path| entries.iter().find(|e| e.path == p).unwrap_or_else(|| panic!("{p:?} is not listed"));
        assert_eq!(by(&newer).name, "Hyperloop");
        assert_eq!(by(&newer).content, Content::Newer(7));
        for (path, want) in [
            (&unknown, "Teleporter"),
            (&junk, "not JSON"),
            (&empty, "not JSON"),
            (&no_format, "no format"),
            (&outside, ""),
            (&overlap, "overlap"),
            (&huge, "box"),
        ] {
            match &by(path).content {
                Content::Broken(why) => assert!(why.contains(want), "{path:?}: {why}"),
                other => panic!("{path:?} should be unreadable, got {other:?}"),
            }
        }
        assert_eq!(by(&unknown).name, "Good", "the name inside is used when it can be read");
        assert_eq!(by(&junk).name, "junk", "the file name stands in otherwise");
        assert!(matches!(by(&dir(&data).join("good.json")).content, Content::Ok(_)));
        assert_eq!(entries.len(), 9, "the text file, the hidden file and the folder are not listed");
        // Listing touched nothing.
        for p in [&newer, &unknown, &junk, &empty, &no_format, &outside, &overlap, &huge] {
            assert!(p.exists());
        }
        assert_eq!(std::fs::read_to_string(&junk).unwrap(), "this is not json {");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn delete_removes_only_json_files() {
        let data = temp("delete");
        let path = save(&data, &sample("Gone")).unwrap();
        let other = dir(&data).join("keep.txt");
        std::fs::write(&other, "x").unwrap();
        assert!(delete(&other).is_err() && other.exists());
        delete(&path).unwrap();
        assert!(!path.exists() && list(&data).is_empty());
        assert!(delete(&path).is_err(), "already gone");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_newer_files_extra_fields_and_missing_optional_ones_are_fine_for_this_format() {
        let data = temp("lenient");
        write(
            &data,
            "plain.json",
            r#"{"format":1,"name":"Plain","extra":[1,2],"w":2,"h":1,"parts":[{"dx":0,"dy":0,"kind":"Belt","dir":"E","future":true},{"dx":1,"dy":0,"kind":"Pole","dir":"N"}]}"#,
        );
        let entries = list(&data);
        match &entries[0].content {
            Content::Ok(bp) => assert_eq!((bp.part_count(), bp.level, bp.w, bp.h), (2, 0, 2, 1)),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&data);
    }
}
