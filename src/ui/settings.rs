//! The settings pane (`c`): a short list of settings over the map. It is centred in the map, so
//! the sidebar beside it stays in view and a change to the sidebar graph shows at once.

use super::*;
use crate::settings::ROWS;

/// Draws the pane in `area`. The game keeps running underneath.
pub(super) fn draw_settings(f: &mut Frame, area: Rect, app: &App) {
    let dim = Style::new().fg(Color::DarkGray);
    let n = ROWS.len() as u16;
    // Borders, the rows, a blank row and the picked row's hint (two lines) if there is room, and the footer.
    let h = (n + 6).min(area.height);
    let r = centered(area, 46, h);
    f.render_widget(Clear, r);
    let block = Block::bordered().title(" Settings ").title_style(Style::new().fg(Color::Cyan));
    let inner = block.inner(r);
    f.render_widget(block, r);
    if inner.width < 12 || inner.height < 2 {
        return;
    }
    let width = inner.width.saturating_sub(2) as usize;
    let body = Rect::new(inner.x + 1, inner.y, width as u16, inner.height - 1);
    let pick = app.settings_pick.min(ROWS.len() - 1);
    // With little height the rows scroll to keep the picked one in view.
    let room = body.height as usize;
    let help: Vec<String> = wrap_words(ROWS[pick].help, width).into_iter().take(2).collect();
    let help_room = room > ROWS.len() + help.len();
    let visible = if help_room { ROWS.len() } else { room.min(ROWS.len()) };
    let start = if pick < visible { 0 } else { pick + 1 - visible };
    let mut lines: Vec<Line> = Vec::new();
    for (i, row) in ROWS.iter().enumerate().skip(start).take(visible) {
        let value = format!("‹ {} ›", (row.value)(&app.settings));
        let name = fit(row.name, width.saturating_sub(value.chars().count() + 1));
        let pad = width.saturating_sub(name.chars().count() + value.chars().count());
        let text = format!("{name}{}{value}", " ".repeat(pad));
        // A setting with no effect right now (the width, when the view isn't Auto) is greyed out.
        let mut style = if (row.enabled)(&app.settings) { Style::new() } else { dim };
        if i == pick {
            style = style.add_modifier(Modifier::REVERSED);
        }
        lines.push(Line::styled(text, style));
    }
    if help_room {
        lines.push(Line::raw(""));
        lines.extend(help.into_iter().map(|l| Line::styled(l, dim)));
    }
    f.render_widget(Paragraph::new(lines), body);
    let foot = Rect::new(inner.x + 1, inner.y + inner.height - 1, width as u16, 1);
    let close = format!("{}/Esc close", app.controls.key(Action::Settings));
    let hints = [(0, "←→ change"), (0, close.as_str()), (1, "↑↓ pick"), (2, "the game keeps running")];
    f.render_widget(Paragraph::new(footer_line(foot.width, &hints, None)), foot);
}
