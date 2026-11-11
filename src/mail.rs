//! Agent mail: AI agents (or a Claude Code hook) drop plain Markdown files into
//! ~/.terminactory/inbox; the game picks them up and shows each as a telegram.
//! Agents write ordinary Markdown, never game text, so a message costs only what it says.
//!
//! File format: any `*.md` file. An optional first line `from: <name>` names the sender.

use std::collections::VecDeque;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use serde::{Deserialize, Serialize};

use crate::content::{Company, Tech};

/// Longest message kept, in characters (an agent's final reply can be long).
const MAX_CHARS: usize = 6000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Who sent it: usually the project folder the agent works in.
    pub from: String,
    /// What happened: "finished", "needs you", or empty for a plain message.
    #[serde(default)]
    pub event: String,
    pub md: String,
    /// When it arrived (seconds since 1970).
    pub at: u64,
    /// A company delivery that came with it, and how much money it brought.
    #[serde(default)]
    pub gift: Option<(Company, f64)>,
    /// Set when it is delivered (1, 2, 3...), so the log can tell which one was read.
    #[serde(default)]
    pub id: u64,
}

/// One line of the telegram log: a message and whether it has been closed with n / Esc yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub msg: Message,
    pub read: bool,
}

pub fn inbox(data_dir: &Path) -> PathBuf {
    data_dir.join("inbox")
}

/// Writes a message into the inbox. Written under a hidden name, then renamed, so the game never reads half a file.
pub fn post(data_dir: &Path, from: &str, event: &str, md: &str) -> io::Result<PathBuf> {
    let dir = inbox(data_dir);
    std::fs::create_dir_all(&dir)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let name = format!("{nanos}-{}.md", std::process::id());
    let tmp = dir.join(format!(".{name}"));
    let mut text = format!("from: {}\n", one_line(from));
    if !event.is_empty() {
        text.push_str(&format!("event: {}\n", one_line(event)));
    }
    text.push_str(md);
    std::fs::write(&tmp, text)?;
    let path = dir.join(name);
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

fn one_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().chars().take(60).collect()
}

/// Takes every message waiting in the inbox, oldest first, and removes the files.
pub fn collect(data_dir: &Path, now: u64) -> Vec<Message> {
    let Ok(entries) = std::fs::read_dir(inbox(data_dir)) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md") && !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.')))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for p in files {
        if let Ok(text) = std::fs::read_to_string(&p) {
            out.push(parse(&text, now));
        }
        let _ = std::fs::remove_file(&p);
    }
    out
}

/// Reads the optional `from:` / `event:` header lines, then the Markdown body.
pub fn parse(text: &str, now: u64) -> Message {
    let (mut from, mut event) = (String::from("your agent"), String::new());
    let mut rest = text;
    for _ in 0..2 {
        let (line, after) = rest.split_once('\n').unwrap_or((rest, ""));
        if let Some(v) = line.strip_prefix("from:") {
            from = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("event:") {
            event = v.trim().to_string();
        } else {
            break;
        }
        rest = after;
    }
    let mut md: String = rest.trim().chars().take(MAX_CHARS).collect();
    if rest.trim().chars().count() > MAX_CHARS {
        md.push_str("\n\n…");
    }
    if md.is_empty() {
        md = "Done.".into();
    }
    Message { from, event, md, at: now, gift: None, id: 0 }
}

/// Turns a Claude Code hook's JSON (Stop or Notification) into (sender, event, Markdown).
/// The Stop hook reuses the agent's own last reply, so it costs no extra tokens.
pub fn from_hook(json: &str) -> Option<(String, String, String)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let from = Path::new(&s("cwd")).file_name().and_then(|n| n.to_str()).unwrap_or("your agent").to_string();
    match s("hook_event_name").as_str() {
        "Notification" => Some((from, "needs you".into(), s("message"))),
        "SubagentStop" => None,
        _ => {
            let mut md = s("last_assistant_message");
            if md.trim().is_empty() {
                md = last_reply(&s("transcript_path")).unwrap_or_default();
            }
            Some((from, "finished".into(), md))
        }
    }
}

/// The text of the last assistant reply in a Claude Code transcript (JSON lines).
fn last_reply(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines().rev() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let content = v.pointer("/message/content")?;
        let parts: Vec<&str> = match content {
            serde_json::Value::String(s) => vec![s.as_str()],
            serde_json::Value::Array(a) => a
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect(),
            _ => vec![],
        };
        let joined = parts.join("\n\n");
        if !joined.trim().is_empty() {
            return Some(joined);
        }
    }
    None
}

/// `tfac msg ...` and `tfac hook`. Returns true if the arguments were a mail command.
/// Prints nothing on success, so an agent that runs it spends no tokens reading output.
pub fn run_cli(args: &[String], data_dir: &Path) -> bool {
    match args.get(1).map(String::as_str) {
        Some("msg") => {
            let mut from = std::env::current_dir()
                .ok()
                .and_then(|d| d.file_name().and_then(|n| n.to_str()).map(String::from))
                .unwrap_or_else(|| "your agent".into());
            let mut words = Vec::new();
            let mut it = args[2..].iter();
            while let Some(a) = it.next() {
                if a == "--from" {
                    if let Some(f) = it.next() {
                        from = f.clone();
                    }
                } else {
                    words.push(a.as_str());
                }
            }
            if matches!(words.as_slice(), ["--help"] | ["-h"]) {
                println!("tfac msg [--from NAME] [TEXT]: sends Markdown to the game as a telegram. \\n is a line break; with no TEXT it reads stdin.");
                return true;
            }
            let md = if words.is_empty() {
                let mut s = String::new();
                let _ = io::stdin().read_to_string(&mut s);
                s
            } else {
                // `\n` in an argument is a line break, so one short command can send several lines.
                words.join(" ").replace("\\n", "\n")
            };
            if let Err(e) = post(data_dir, &from, "", &md) {
                eprintln!("tfac msg: {e}");
                std::process::exit(1);
            }
            true
        }
        Some("hook") => {
            // A hook must never get in the agent's way: always exit quietly.
            let mut s = String::new();
            let _ = io::stdin().read_to_string(&mut s);
            if let Some((from, event, md)) = from_hook(&s) {
                let _ = post(data_dir, &from, &event, &md);
            }
            true
        }
        _ => false,
    }
}

/// A company's letter as a telegram: its words, then what it opens for research.
/// `key` is the key that opened the company, which picks the text where a company has two.
pub fn letter(c: Company, key: crate::content::Made, now: u64) -> Message {
    let mut md = format!("\"{}\"", c.letter(key));
    let techs: Vec<String> =
        Tech::ALL.iter().filter(|t| t.company() == c).map(|t| format!("- {} ({})", t.name(), t.category().name())).collect();
    if !techs.is_empty() {
        md.push_str("\n\n**Unlocked for research**\n");
        md.push_str(&techs.join("\n"));
    }
    Message { from: c.name().to_string(), event: "letter".into(), md, at: now, gift: None, id: 0 }
}

/// A delivery's note, in the sending company's voice.
pub fn gift_note(c: Company) -> &'static str {
    match c {
        Company::Hollowmere => "Hollowmere left a cart of coal money at the gate. \"Thought you'd been waiting.\"",
        Company::Brannock => "The Brannock sisters dropped off a little something. \"Don't spend it on Swiftline.\"",
        Company::Ashgrove => "Ashgrove sent a cheque, on very good paper.",
        Company::Cinderhall => "Cinderhall sent cash and, unasked, a sack of waste. The cash is what counts.",
        Company::Wellbourne => "Wellbourne sent payment by pipe. It arrived quietly, and slightly damp.",
        Company::Kettering => "Kettering & Vane sent two cheques, then argued about which one to send. You kept both halves.",
        Company::Pillar => "Pillar & Sons sent a payment rated for twice its load.",
        Company::Underhill => "Underhill dug up a tin of coins and sent it with a cheerful note.",
        Company::Gearwright => "Gearwright sent a payment accurate to the penny.",
        Company::Swiftline => "Swiftline couriered a bonus. It arrived before they posted it.",
        Company::Ampleworth => "Ampleworth sent a payment in a small crate. The crate hums faintly.",
        Company::LatticeFinch => "Lattice & Finch sent a cheque, and a note on the correct way to fold it.",
    }
}

// ---- Markdown to terminal lines -------------------------------------------------

const HEAD: Color = Color::Indexed(220);
const CODE: Color = Color::Indexed(152);
const DIM: Color = Color::Indexed(244);

/// Renders Markdown (headings, lists, tasks, quotes, code, tables, rules, **bold**, *italic*,
/// `code`, [links](...)) as styled lines wrapped to `width`, with wrapped rows lined up under
/// their text (not under the bullet). Anything unknown shows as plain text.
pub fn render(md: &str, width: u16) -> Vec<Line<'static>> {
    lines(md).iter().flat_map(|l| wrap(l, width)).collect()
}

fn lines(md: &str) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut fence = false;
    let mut comment = false;
    for raw in md.lines() {
        let line = raw.trim_end();
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            out.push(Line::from(Span::styled(format!("   {line}"), Style::new().fg(CODE))));
            continue;
        }
        if comment || t.starts_with("<!--") {
            comment = !t.contains("-->");
            continue;
        }
        if t.is_empty() {
            if out.last().is_some_and(|l| l.width() > 0) {
                out.push(Line::raw(""));
            }
            continue;
        }
        let indent = " ".repeat(1 + (line.len() - t.len()) / 2 * 2);
        if let Some((level, text)) = heading(t) {
            if out.last().is_some_and(|l| l.width() > 0) {
                out.push(Line::raw(""));
            }
            let style =
                if level == 1 { Style::new().fg(HEAD).add_modifier(Modifier::BOLD) } else { Style::new().add_modifier(Modifier::BOLD) };
            let mut spans = vec![Span::raw(" ")];
            spans.extend(inline(text, style));
            out.push(Line::from(spans));
            continue;
        }
        if t.chars().all(|c| matches!(c, '-' | '*' | '_' | ' ')) && t.chars().filter(|c| *c != ' ').count() >= 3 {
            out.push(Line::from(Span::styled(" ────────────", Style::new().fg(DIM))));
            continue;
        }
        if let Some(q) = t.strip_prefix('>') {
            let mut spans = vec![Span::styled(" │ ", Style::new().fg(DIM))];
            spans.extend(inline(q.trim_start(), Style::new().fg(DIM).add_modifier(Modifier::ITALIC)));
            out.push(Line::from(spans));
            continue;
        }
        if t.starts_with('|') {
            // Tables: drop the |---| row, show cells split by a thin bar.
            if t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                continue;
            }
            let cells: Vec<&str> = t.trim_matches('|').split('|').map(str::trim).collect();
            let mut spans = vec![Span::raw(" ")];
            for (i, c) in cells.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" │ ", Style::new().fg(DIM)));
                }
                spans.extend(inline(c, Style::new()));
            }
            out.push(Line::from(spans));
            continue;
        }
        if let Some((mark, text)) = list_item(t) {
            let mut spans = vec![Span::raw(indent), Span::styled(format!("{mark} "), Style::new().fg(HEAD))];
            spans.extend(inline(text, Style::new()));
            out.push(Line::from(spans));
            continue;
        }
        let mut spans = vec![Span::raw(" ")];
        spans.extend(inline(t, Style::new()));
        out.push(Line::from(spans));
    }
    while out.last().is_some_and(|l| l.width() == 0) {
        out.pop();
    }
    out
}

fn heading(t: &str) -> Option<(usize, &str)> {
    let level = t.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&level) && t[level..].starts_with(' ') {
        Some((level, t[level..].trim().trim_end_matches('#').trim_end()))
    } else {
        None
    }
}

/// "- x", "* x", "+ x", "1. x", "- [ ] x", "- [x] x" -> (marker, text).
fn list_item(t: &str) -> Option<(String, &str)> {
    for b in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(b) {
            if let Some(r) = rest.strip_prefix("[ ] ") {
                return Some(("[ ]".into(), r));
            }
            if let Some(r) = rest.strip_prefix("[x] ").or_else(|| rest.strip_prefix("[X] ")) {
                return Some(("[x]".into(), r));
            }
            return Some(("•".into(), rest));
        }
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < 4 {
        let rest = &t[digits..];
        if let Some(r) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some((format!("{}.", &t[..digits]), r));
        }
    }
    None
}

/// Inline Markdown: **bold**, __bold__, *italic*, `code`, ~~strike~~, [text](url).
/// `_` alone is left as is, since agents write snake_case names all the time.
fn inline(s: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut buf = String::new();
    let (mut bold, mut italic, mut strike) = (false, false, false);
    let chars: Vec<char> = s.chars().collect();
    let style = |b: bool, i: bool, k: bool| {
        let mut st = base;
        if b {
            st = st.add_modifier(Modifier::BOLD);
        }
        if i {
            st = st.add_modifier(Modifier::ITALIC);
        }
        if k {
            st = st.add_modifier(Modifier::CROSSED_OUT);
        }
        st
    };
    let mut i = 0;
    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut buf), style(bold, italic, strike)));
            }
        };
    }
    let closes = |from: usize, pat: &[char]| -> bool { (from..chars.len()).any(|j| chars[j..].starts_with(pat)) };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '\\' && next.is_some_and(|n| n.is_ascii_punctuation()) {
            buf.push(next.unwrap());
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(end) = (i + 1..chars.len()).find(|&j| chars[j] == '`') {
                flush!();
                let code: String = chars[i + 1..end].iter().collect();
                spans.push(Span::styled(code, base.fg(CODE)));
                i = end + 1;
                continue;
            }
        }
        if ((c == '*' && next == Some('*')) || (c == '_' && next == Some('_'))) && (bold || closes(i + 2, &[c, c])) {
            flush!();
            bold = !bold;
            i += 2;
            continue;
        }
        if c == '~' && next == Some('~') && (strike || closes(i + 2, &['~', '~'])) {
            flush!();
            strike = !strike;
            i += 2;
            continue;
        }
        if c == '*' && (italic || (next.is_some_and(|n| !n.is_whitespace()) && closes(i + 1, &['*']))) {
            flush!();
            italic = !italic;
            i += 1;
            continue;
        }
        if c == '[' {
            if let Some(mid) = (i + 1..chars.len()).find(|&j| chars[j] == ']') {
                if chars.get(mid + 1) == Some(&'(') {
                    if let Some(end) = (mid + 2..chars.len()).find(|&j| chars[j] == ')') {
                        flush!();
                        let text: String = chars[i + 1..mid].iter().collect();
                        spans.push(Span::styled(text, style(bold, italic, strike).add_modifier(Modifier::UNDERLINED)));
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        buf.push(c);
        i += 1;
    }
    flush!();
    spans
}

/// Where a wrapped row should start: under the text, past any indent and list marker or quote bar.
fn hang_of(text: &[char]) -> usize {
    let lead = text.iter().take_while(|c| **c == ' ').count();
    let rest: String = text[lead..].iter().take(4).collect();
    for m in ["• ", "[ ] ", "[x] ", "│ "] {
        if rest.starts_with(m) {
            return lead + m.chars().count();
        }
    }
    let digits = text[lead..].iter().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && text.get(lead + digits) == Some(&'.') && text.get(lead + digits + 1) == Some(&' ') {
        return lead + digits + 2;
    }
    lead
}

/// Word-wraps one styled line to `width` columns; continuation rows get a hanging indent.
pub fn wrap(line: &Line<'static>, width: u16) -> Vec<Line<'static>> {
    wrap_with(line, width, None)
}

/// Like `wrap`, but continuation rows start at column `hang` (e.g. past a "Made by" label).
pub fn wrap_with(line: &Line<'static>, width: u16, hang: Option<usize>) -> Vec<Line<'static>> {
    let width = width.max(8) as usize;
    let cells: Vec<(char, Style)> =
        line.spans.iter().flat_map(|s| s.content.chars().map(move |c| (c, line.style.patch(s.style)))).collect();
    if cells.len() <= width {
        return vec![line.clone()];
    }
    let text: Vec<char> = cells.iter().map(|c| c.0).collect();
    let hang = hang.unwrap_or_else(|| hang_of(&text)).min(width / 2);
    let mut rows: Vec<Vec<(char, Style)>> = vec![Vec::new()];
    let mut i = 0;
    while i < cells.len() {
        // The next token: a run of spaces or a run of non-spaces.
        let space = cells[i].0 == ' ';
        let end = (i..cells.len()).find(|&j| (cells[j].0 == ' ') != space).unwrap_or(cells.len());
        let tok = &cells[i..end];
        i = end;
        let first = rows.len() == 1;
        let row = rows.last_mut().unwrap();
        let fresh = !first && row.len() == hang;
        if space {
            if !fresh && row.len() + tok.len() <= width {
                row.extend_from_slice(tok);
            }
            continue;
        }
        let mut tok = tok;
        loop {
            let start = if rows.len() > 1 { hang } else { 0 };
            let row = rows.last_mut().unwrap();
            if row.len() + tok.len() <= width {
                row.extend_from_slice(tok);
                break;
            }
            if row.len() > start {
                // Doesn't fit: drop trailing spaces and start a new row.
                while row.last().is_some_and(|c| c.0 == ' ') {
                    row.pop();
                }
                rows.push(vec![(' ', Style::new()); hang]);
                continue;
            }
            // A word longer than a whole row: split it.
            let room = width - row.len();
            row.extend_from_slice(&tok[..room]);
            tok = &tok[room..];
            rows.push(vec![(' ', Style::new()); hang]);
        }
    }
    rows.into_iter()
        .map(|row| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut buf = String::new();
            let mut cur = None;
            for (c, st) in row {
                if cur.is_some_and(|s| s != st) {
                    spans.push(Span::styled(std::mem::take(&mut buf), cur.unwrap()));
                }
                cur = Some(st);
                buf.push(c);
            }
            if let Some(st) = cur {
                spans.push(Span::styled(buf, st));
            }
            Line::from(spans)
        })
        .collect()
}

/// Messages waiting to be read (kept in the save, so none are lost on quit).
pub type Queue = VecDeque<Message>;

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> String {
        lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn markdown_renders_plainly() {
        let md = "# Auth fix done\n\nTests **pass**, see `src/auth.rs`.\n\n- added retry\n  - nested\n- [x] lint\n1. one\n> careful\n\n```\nlet x = 1;\n```\n| a | b |\n|---|---|\n| 1 | 2 |\n---\nsee [the PR](https://x.y) and my_snake_case";
        let out = text(&render(md, 200));
        assert!(out.contains(" Auth fix done"));
        assert!(out.contains("Tests pass, see src/auth.rs."));
        assert!(out.contains(" • added retry"));
        assert!(out.contains("   • nested"));
        assert!(out.contains("[x] lint"));
        assert!(out.contains("1. one"));
        assert!(out.contains("│ careful"));
        assert!(out.contains("   let x = 1;"));
        assert!(out.contains(" 1 │ 2"));
        assert!(!out.contains("---"));
        assert!(out.contains("see the PR and my_snake_case"));
        assert!(!out.contains("```"));
    }

    #[test]
    fn wrapped_rows_line_up_under_their_text() {
        let out = render("- alpha beta gamma delta epsilon zeta eta theta\n\nplain words that keep going on and on", 20);
        let rows: Vec<String> = out.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();
        assert_eq!(rows[0], " • alpha beta gamma");
        assert!(rows[1].starts_with("   delta"), "{rows:?}");
        assert!(rows.iter().all(|r| r.chars().count() <= 20), "{rows:?}");
        let plain = rows.iter().position(|r| r.starts_with(" plain")).unwrap();
        assert!(rows[plain + 1].starts_with(" ") && !rows[plain + 1].starts_with("  "), "{rows:?}");
    }

    #[test]
    fn header_lines_are_optional() {
        let m = parse("from: api\nevent: finished\n## Hi", 5);
        assert_eq!((m.from.as_str(), m.event.as_str(), m.md.as_str()), ("api", "finished", "## Hi"));
        let m = parse("just text", 5);
        assert_eq!((m.from.as_str(), m.md.as_str()), ("your agent", "just text"));
    }

    #[test]
    fn hook_json_becomes_a_message() {
        let j = r#"{"hook_event_name":"Stop","cwd":"/Users/a/proj","last_assistant_message":"Done: **3 tests** fixed"}"#;
        assert_eq!(from_hook(j), Some(("proj".into(), "finished".into(), "Done: **3 tests** fixed".into())));
        let j = r#"{"hook_event_name":"Notification","cwd":"/x/web","message":"Claude needs your permission to use Bash"}"#;
        assert_eq!(from_hook(j).unwrap().1, "needs you");
        assert_eq!(from_hook(r#"{"hook_event_name":"SubagentStop"}"#), None);
    }

    #[test]
    fn hook_falls_back_to_the_transcript() {
        let path = std::env::temp_dir().join(format!("tfac-transcript-{}.jsonl", std::process::id()));
        std::fs::write(
            &path,
            concat!(
                r#"{"type":"user","message":{"content":"hi"}}"#,
                "\n",
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"All done."},{"type":"tool_use"}]}}"#,
                "\n",
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use"}]}}"#,
                "\n",
            ),
        )
        .unwrap();
        let j = format!(r#"{{"hook_event_name":"Stop","cwd":"/p/q","transcript_path":"{}"}}"#, path.display());
        assert_eq!(from_hook(&j).unwrap().2, "All done.");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn post_then_collect() {
        let dir = std::env::temp_dir().join(format!("tfac-mail-{}", std::process::id()));
        post(&dir, "proj", "finished", "# One").unwrap();
        post(&dir, "proj", "", "two").unwrap();
        std::fs::write(inbox(&dir).join(".half.md"), "partial").unwrap();
        let got = collect(&dir, 1);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].md, "# One");
        assert_eq!(got[1].event, "");
        assert!(collect(&dir, 2).is_empty());
        std::fs::remove_dir_all(dir).ok();
    }
}
