//! Numbered saves: save1, save2, ... in ~/.terminactory/saves.
//! `tfac` continues the one played last, `tfac save2` (or `tfac 2`) opens one,
//! `tfac --new` starts the next number, and `tfac --saves` lists them.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct SaveInfo {
    pub name: String,
    pub path: PathBuf,
    pub modified: SystemTime,
}

pub fn dir(data: &Path) -> PathBuf {
    data.join("saves")
}

/// The number in "save12", if the name is one of ours.
fn number(name: &str) -> Option<u32> {
    name.strip_prefix("save")?.parse().ok()
}

/// What to tell the player when a save is from another version of the game and can't be opened.
pub fn version_problem(path: &Path, name: &str) -> Option<String> {
    let v = crate::sim::World::file_version(path)?;
    if v < crate::sim::SAVE_VERSION {
        Some(format!(
            "{name} is from an older version of Terminactory and can't be opened. It's still at {}.\nStart a fresh game with: tfac --new",
            path.display()
        ))
    } else if v > crate::sim::SAVE_VERSION {
        Some(format!("{name} was made by a newer version of Terminactory and can't be opened. Update the game, or start a fresh one with: tfac --new"))
    } else {
        None
    }
}

/// Every numbered save, lowest number first.
pub fn list(data: &Path) -> Vec<SaveInfo> {
    let Ok(entries) = std::fs::read_dir(dir(data)) else {
        return Vec::new();
    };
    let mut v: Vec<SaveInfo> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_stem()?.to_str()?.to_string();
            (path.extension()? == "json" && number(&name).is_some()).then_some(())?;
            let modified = e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            Some(SaveInfo { name, path, modified })
        })
        .collect();
    v.sort_by_key(|s| number(&s.name));
    v
}

/// The save played last (the most recently written).
pub fn latest(data: &Path) -> Option<SaveInfo> {
    list(data).into_iter().max_by_key(|s| s.modified)
}

/// The name for a new save: one past the highest number.
pub fn next_name(data: &Path) -> String {
    let n = list(data).iter().filter_map(|s| number(&s.name)).max().unwrap_or(0);
    format!("save{}", n + 1)
}

/// What the player typed ("save2", "2") as a save name.
pub fn parse_name(arg: &str) -> Option<String> {
    if let Ok(n) = arg.parse::<u32>() {
        return Some(format!("save{n}"));
    }
    number(arg).map(|n| format!("save{n}"))
}

pub fn path_of(data: &Path, name: &str) -> PathBuf {
    dir(data).join(format!("{name}.json"))
}

/// While a save is open, a `.lock` file beside it holds the game's process id, so a second
/// window can't open the same save and overwrite it. Removed when the game closes.
pub struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Takes the lock for a save, or says which process has it open. A lock left behind by a
/// game that crashed (its process is gone) is taken over.
pub fn lock(save: &Path) -> Result<Lock, u32> {
    let path = save.with_extension("lock");
    if let Some(pid) = std::fs::read_to_string(&path).ok().and_then(|t| t.trim().parse::<u32>().ok()) {
        if pid != std::process::id() && alive(pid) {
            return Err(pid);
        }
    }
    let _ = std::fs::write(&path, std::process::id().to_string());
    Ok(Lock(path))
}

/// "3 minutes ago", "2 days ago".
fn ago(t: SystemTime) -> String {
    let s = SystemTime::now().duration_since(t).map_or(0, |d| d.as_secs());
    let (n, unit) = match s {
        0..=59 => return "just now".into(),
        60..=3599 => (s / 60, "minute"),
        3600..=86_399 => (s / 3600, "hour"),
        _ => (s / 86_400, "day"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// A whole number with thousands marks: 17,687.
fn grouped(v: f64) -> String {
    let s = (v.max(0.0).floor() as u64).to_string();
    let mut out = String::new();
    for (k, ch) in s.chars().enumerate() {
        if k > 0 && (s.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// The list `tfac --saves` prints.
pub fn describe(data: &Path) -> String {
    let saves = list(data);
    if saves.is_empty() {
        return "No saves yet. Start one with: tfac\n".into();
    }
    let last = latest(data).map(|s| s.name);
    let mut out = format!("Your saves (in {}):\n", dir(data).display());
    for s in &saves {
        let old = crate::sim::World::file_version(&s.path).is_some_and(|v| v < crate::sim::SAVE_VERSION);
        let stats = match crate::sim::World::load(&s.path) {
            _ if old => "(old version)".to_string(),
            Ok(w) => {
                let secs = w.tick / crate::sim::TPS;
                format!(
                    "{:<9} {:>7} research   {}h {:02}m played",
                    format!("${}", grouped(w.econ.money)),
                    grouped(w.econ.research),
                    secs / 3600,
                    (secs % 3600) / 60
                )
            }
            Err(_) => "(can't be read)".into(),
        };
        let open =
            std::fs::read_to_string(s.path.with_extension("lock")).ok().and_then(|t| t.trim().parse::<u32>().ok()).is_some_and(alive);
        let mark = if open {
            "   (open now)"
        } else if last.as_deref() == Some(s.name.as_str()) {
            "   ← tfac opens this"
        } else {
            ""
        };
        out += &format!("  {:<7} {stats}   last played {}{mark}\n", s.name, ago(s.modified));
    }
    out += "\nOpen one: tfac save1   ·   Start a new one: tfac --new\n";
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_are_numbered() {
        let data = std::env::temp_dir().join(format!("tfac-saves-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(dir(&data)).unwrap();
        assert_eq!(next_name(&data), "save1");
        std::fs::write(path_of(&data, "save1"), "{}").unwrap();
        std::fs::write(path_of(&data, "save3"), "not a save").unwrap();
        std::fs::write(dir(&data).join("demo.json"), "{}").unwrap();
        let names: Vec<String> = list(&data).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["save1", "save3"], "the demo isn't a numbered save");
        assert_eq!(next_name(&data), "save4");
        assert_eq!(latest(&data).unwrap().name, "save3");
        assert_eq!(parse_name("2").as_deref(), Some("save2"));
        assert_eq!(parse_name("save12").as_deref(), Some("save12"));
        assert_eq!(parse_name("hello"), None);
        assert!(describe(&data).contains("save3") && describe(&data).contains("can't be read"));
        // A save without a version is from before versions: listed as old, not read.
        let line = |name: &str| describe(&data).lines().find(|l| l.contains(name)).unwrap().to_string();
        assert!(line("save1").contains("(old version)") && !line("save1").contains("research"), "{}", line("save1"));
        // A save open in this process is locked to others; a stale lock is taken over.
        let save = path_of(&data, "save3");
        std::fs::write(save.with_extension("lock"), "999999").unwrap();
        let held = lock(&save).expect("stale lock is taken over");
        assert_eq!(std::fs::read_to_string(save.with_extension("lock")).unwrap(), std::process::id().to_string());
        drop(held);
        assert!(!save.with_extension("lock").exists(), "closing removes the lock");
        let parent = std::os::unix::process::parent_id();
        std::fs::write(save.with_extension("lock"), parent.to_string()).unwrap();
        assert_eq!(lock(&save).err(), Some(parent), "a running game keeps its lock");
        assert!(describe(&data).contains("(open now)"));
        let _ = std::fs::remove_dir_all(&data);
    }
}
