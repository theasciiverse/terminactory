//! The telegram log: every telegram and letter received, newest first, unread ones marked.

use super::*;

/// "5m", "3h", "2d": how long ago a message came.
fn age(at: u64, now: u64) -> String {
    let s = now.saturating_sub(at);
    match s {
        0..=59 => "now".into(),
        60..=3599 => format!("{}m", s / 60),
        3600..=86399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

/// The first line of a message's text, without Markdown marks.
fn preview(md: &str) -> String {
    let line = md.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let line = line.trim_start_matches(['#', '>', '-', '*', ' ']);
    line.chars().filter(|c| !matches!(c, '*' | '`')).collect()
}

/// One row of the log, `width` columns wide: marker, sender, kind, what it says, age.
fn row(e: &crate::mail::LogEntry, width: usize, now: u64) -> Vec<Span<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let (mark, name_style) = if e.read {
        (Span::styled("  ", dim), Style::new())
    } else {
        (Span::styled("● ", Style::new().fg(KEY_ITEM)), Style::new().add_modifier(Modifier::BOLD))
    };
    let when = if width >= 36 { format!(" {:>3}", age(e.msg.at, now)) } else { String::new() };
    let room = width.saturating_sub(2 + when.chars().count());
    let from = fit(&e.msg.from, room.min(22));
    let event = if e.msg.event.is_empty() { String::new() } else { format!(" · {}", e.msg.event) };
    let used = from.chars().count() + event.chars().count();
    let text = if used + 4 < room { format!(" {}", fit(&preview(&e.msg.md), room - used - 1)) } else { String::new() };
    let pad = room.saturating_sub(used + text.chars().count());
    vec![
        mark,
        Span::styled(from, name_style),
        Span::styled(fit(&event, room.saturating_sub(1)), Style::new().fg(Color::Cyan)),
        Span::styled(text, dim),
        Span::raw(" ".repeat(pad)),
        Span::styled(when, dim),
    ]
}

/// Draws the log over the map. The game keeps running underneath.
pub fn draw_log(f: &mut Frame, area: Rect, app: &mut App) {
    let head = Style::new().fg(KEY_ITEM).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(Color::DarkGray);
    f.render_widget(Clear, area);
    let w = &app.world;
    let unread = w.unread();
    let title = if unread > 0 { format!(" Telegram log · {unread} unread ") } else { " Telegram log ".to_string() };
    // No border of its own: it sits inside the map's.
    let block = Block::new().title(title).title_style(head);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width < 10 || inner.height < 3 {
        return;
    }
    let key = app.controls.key(Action::MailLog);
    let n = w.log.len();
    let [list, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    if n == 0 {
        f.render_widget(
            Paragraph::new(" Nothing yet. Agent telegrams and company letters are kept here.").style(dim).wrap(Wrap { trim: true }),
            list,
        );
    } else {
        app.log_pick = app.log_pick.min(n - 1);
        let height = list.height as usize;
        // Keep the picked row in view.
        let top = app.log_pick.saturating_sub(height / 2).min(n.saturating_sub(height));
        let now = crate::sim::now_secs();
        let lines: Vec<Line> = w
            .log
            .iter()
            .rev()
            .enumerate()
            .skip(top)
            .take(height)
            .map(|(i, e)| {
                let mut spans = row(e, list.width as usize, now);
                if i == app.log_pick {
                    for s in &mut spans {
                        s.style = s.style.add_modifier(Modifier::REVERSED);
                    }
                }
                Line::from(spans)
            })
            .collect();
        f.render_widget(Paragraph::new(lines), list);
    }
    let close = format!("{key}/Esc close");
    let hints = [(2, "↑↓ pick"), (1, "Enter read"), (0, close.as_str())];
    let height = list.height as usize;
    let more = if n > height { Some("more ▲▼") } else { None };
    f.render_widget(
        Paragraph::new(footer_line(foot.width.saturating_sub(1), &hints, more)),
        Rect { x: foot.x + 1, width: foot.width.saturating_sub(1), ..foot },
    );
}
