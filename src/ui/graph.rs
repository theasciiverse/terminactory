//! The sidebar's earnings charts (full view only): money, research or both, stacked under the
//! build menu in whatever room the menu leaves. They give way before anything else does: a short
//! sidebar loses the graph first, and it never reaches into the telegram's box below it.

use super::*;
use crate::settings::GraphShow;

/// The tallest a chart box is, borders included.
const MAX_H: u16 = 8;
/// The shortest: borders, two rows of bars and the time axis.
const MIN_H: u16 = 5;

/// Splits the sidebar `area` (the telegram already taken off the bottom) into the panel for the
/// build menu and the chart boxes under it (each with whether it charts research).
pub(super) fn split(app: &mut App, area: Rect) -> (Rect, Vec<(Rect, bool)>) {
    let show = app.settings.sidebar_graph;
    if show == GraphShow::Off || area.width < 20 {
        return (area, Vec::new());
    }
    let room = area.height.saturating_sub(side_need(app, area.width, area.height));
    // Both stack when each gets a whole box; with less room only money shows.
    let charts: Vec<bool> = match show {
        GraphShow::Both if room >= 2 * MIN_H => vec![false, true],
        GraphShow::Both | GraphShow::Money => vec![false],
        _ => vec![true],
    };
    let each = (room / charts.len() as u16).min(MAX_H);
    if each < MIN_H {
        return (area, Vec::new());
    }
    let total = each * charts.len() as u16;
    let panel = Rect::new(area.x, area.y, area.width, area.height - total);
    let boxes: Vec<(Rect, bool)> =
        charts.iter().enumerate().map(|(i, r)| (Rect::new(area.x, panel.bottom() + i as u16 * each, area.width, each), *r)).collect();
    app.graph_areas = boxes.iter().map(|b| b.0).collect();
    (panel, boxes)
}

pub(super) fn draw(f: &mut Frame, boxes: &[(Rect, bool)], app: &App) {
    for (r, research) in boxes {
        let colour = if *research { RESEARCH } else { Color::Green };
        let inner_w = r.width.saturating_sub(2) as usize;
        let bar_rows = r.height.saturating_sub(3) as usize;
        let (title, lines) = history::sidebar_chart(app, *research, inner_w, bar_rows);
        let block = Block::bordered().title(format!(" {} ", fit(&title, inner_w.saturating_sub(2)))).title_style(Style::new().fg(colour));
        f.render_widget(Paragraph::new(lines).block(block), *r);
    }
}
