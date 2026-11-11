//! The blueprint library (`l`): saved blueprints in a list over the map. Like the production panel
//! it has no border of its own and the game keeps running underneath.

use super::*;
use crate::library::Content;

/// The line above the list: what the hand holds, or the last copy, and that `f` saves it.
fn hand_text(app: &App, width: usize) -> (String, bool) {
    let key = app.controls.key(Action::Setup);
    let (bp, label) = match app.savable() {
        Some((bp, true)) => (bp, "In hand"),
        Some((bp, false)) => (bp, "Last copy"),
        None => {
            let copy = app.controls.key(Action::Area);
            return (fit(&format!(" Nothing in your hand yet: copy an area ({copy}, then {copy} again) to save it here"), width), false);
        }
    };
    let n = bp.part_count();
    let long = format!(" {label}: {} · {}x{} · {n} part{} · {key} saves it", bp.name, bp.w, bp.h, if n == 1 { "" } else { "s" });
    let short = format!(" {label}: {}x{} · {n} · {key} saves", bp.w, bp.h);
    (if long.chars().count() <= width { long } else { fit(&short, width) }, true)
}

/// One row of the list, `width` columns wide. Columns drop from the right as it narrows: the cost,
/// then the size, then the part count.
fn row(app: &App, e: &crate::library::Entry, width: usize) -> (Vec<Span<'static>>, bool) {
    let dim = Style::new().fg(Color::DarkGray);
    let (cols, note, usable): (Vec<String>, Option<String>, bool) = match &e.content {
        Content::Ok(bp) => {
            let n = bp.part_count();
            let note = bp.first_locked(&app.world.progress).map(|k| format!("({} locked)", k.name()));
            (vec![format!("{n} part{}", if n == 1 { "" } else { "s" }), format!("{}x{}", bp.w, bp.h), money_text(bp.cost())], note, true)
        }
        Content::Newer(_) => (Vec::new(), Some("(from a newer game)".into()), false),
        Content::Broken(_) => (Vec::new(), Some("(can't be read)".into()), false),
    };
    // Keep at least 14 columns for the name; spend the rest on columns, widest set first.
    let mut cols = cols;
    let total = |c: &[String]| c.iter().map(|s| s.chars().count() + 2).sum::<usize>();
    while !cols.is_empty() && width < 14 + total(&cols) {
        cols.pop();
    }
    let room = width.saturating_sub(total(&cols));
    // A note ("(Kiln locked)") follows the name when it fits there.
    let name = match &note {
        Some(n) if e.name.chars().count() + 1 + n.chars().count() <= room => format!("{} {n}", e.name),
        Some(n) if room > n.chars().count() + 6 => format!("{} {n}", fit(&e.name, room - n.chars().count() - 1)),
        _ => fit(&e.name, room),
    };
    let pad = room.saturating_sub(name.chars().count());
    let mut spans = vec![Span::raw(name), Span::raw(" ".repeat(pad))];
    for c in &cols {
        spans.push(Span::styled(format!("  {c}"), dim));
    }
    let locked = note.is_some();
    (spans, usable && !locked)
}

/// Draws the library in `area` (the map).
pub(super) fn draw_library(f: &mut Frame, area: Rect, app: &mut App) {
    let dim = Style::new().fg(Color::DarkGray);
    let head = Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD);
    f.render_widget(Clear, area);
    let Some(lib) = app.library.as_ref() else { return };
    let (n, pick, typing, armed) = (lib.entries.len(), lib.pick.min(lib.entries.len().saturating_sub(1)), lib.typing.clone(), lib.armed);
    let title = if area.width >= 64 { " Library · saved in ~/.terminactory/blueprints " } else { " Library " };
    let block = Block::new().title(title).title_style(head);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if let Some(l) = app.library.as_mut() {
        l.rows.clear();
        l.pick = pick;
    }
    if inner.width < 12 || inner.height < 3 {
        return;
    }
    let width = inner.width.saturating_sub(2) as usize;
    let [top, list, foot] = Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(Rect {
        x: inner.x + 1,
        width: inner.width - 2,
        ..inner
    });
    let (hand, savable) = hand_text(app, width);
    f.render_widget(Paragraph::new(hand).style(if savable { Style::new() } else { dim }), top);
    let height = list.height as usize;
    if n == 0 {
        let text = " No blueprints saved yet. Files you drop into the folder show up here.";
        f.render_widget(Paragraph::new(text).style(dim).wrap(Wrap { trim: true }), list);
    } else {
        // Keep the picked row in view.
        let first = pick.saturating_sub(height / 2).min(n.saturating_sub(height));
        let mut lines: Vec<Line> = Vec::new();
        let mut hits = Vec::new();
        let entries = app.library.as_ref().map(|l| l.entries.clone()).unwrap_or_default();
        for (i, e) in entries.iter().enumerate().skip(first).take(height) {
            let (mut spans, usable) = row(app, e, width.saturating_sub(2));
            spans.insert(0, Span::raw(if i == pick { "› " } else { "  " }));
            let style = if usable { Style::new() } else { dim };
            for s in &mut spans {
                s.style = s.style.patch(if s.style.fg.is_some() { Style::new() } else { style });
                if i == pick {
                    s.style = s.style.add_modifier(Modifier::REVERSED);
                }
            }
            hits.push((Rect::new(list.x, list.y + (i - first) as u16, list.width, 1), i));
            lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(lines), list);
        if let Some(l) = app.library.as_mut() {
            l.rows = hits;
        }
    }
    // The foot: typing a name, a delete waiting for its second press, or the keys.
    let line = if let Some(name) = typing {
        let enter = app.controls.key(Action::Place);
        let hint = format!("  {enter} saves · Esc cancels");
        let typed = format!(" Name: {name}█");
        let room = foot.width as usize;
        let typed = if typed.chars().count() + hint.chars().count() <= room { typed + &hint } else { fit(&typed, room) };
        Line::styled(typed, Style::new().fg(Color::Yellow))
    } else if armed {
        let name = app.library.as_ref().and_then(|l| l.entries.get(pick)).map_or(String::new(), |e| e.name.clone());
        let key = app.controls.key(Action::Remove);
        Line::styled(
            fit(&format!(" {key} again deletes {name} for good · any other key keeps it"), foot.width as usize),
            Style::new().fg(Color::Yellow),
        )
    } else {
        let close = if foot.width < 44 { "Esc close".to_string() } else { format!("{}/Esc close", app.controls.key(Action::Library)) };
        let take = format!("{} take", app.controls.key(Action::Place));
        let save = format!("{} save hand", app.controls.key(Action::Setup));
        let del = format!("{} delete", app.controls.key(Action::Remove));
        let hints =
            [(3, "↑↓ pick"), (1, take.as_str()), (1, save.as_str()), (2, del.as_str()), (0, close.as_str()), (4, "the game keeps running")];
        let more = if n > height { Some("more ▲▼") } else { None };
        footer_line(foot.width, &hints, more)
    };
    f.render_widget(Paragraph::new(line), foot);
}
