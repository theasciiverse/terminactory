//! The production panel's History tab: earnings charts over seconds, minutes, hours or days.
//! Built as plain lines, so it scrolls like the Production tab when the pane is short.
//! (No calendar of played days: the game should never feel like a commitment.)

use super::*;
use crate::app::StatsHit;
use crate::history::{self as hist, civil, Point, Range, MONTHS};

/// Bars for an eighth to a whole row.
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
/// A stretch with nothing earned.
const FAINT: Color = Color::Indexed(238);

pub(super) struct Page {
    pub lines: Vec<Line<'static>>,
    /// (line, column, width, what a click there does), relative to the page.
    pub hits: Vec<(usize, usize, usize, StatsHit)>,
}

/// Rows of bars in each chart on a page `height` rows tall: one chart takes the page (less the
/// range pills, its summary and its axis), two stacked share it.
fn chart_rows(height: usize) -> usize {
    if stacked(height) {
        (height.saturating_sub(7) / 2).clamp(4, 16)
    } else {
        height.saturating_sub(4).clamp(4, 24)
    }
}

/// Whether a page `height` rows tall shows money and research stacked: both charts (a summary,
/// at least five rows of bars and an axis each) fit without scrolling.
pub(super) fn stacked(height: usize) -> bool {
    height >= 17
}

/// The History tab for a body `width` columns by `height` rows (taller pages scroll).
pub(super) fn page(app: &mut App, width: usize, height: usize) -> Page {
    let dim = Style::new().fg(Color::DarkGray);
    let (now, off) = clock(app);
    let h = &app.world.history;
    // No earnings anywhere yet: a friendly note instead of an empty chart.
    if h.days.iter().all(|b| b.money <= 0.0 && b.research <= 0.0) {
        let lines = vec![
            Line::raw(""),
            Line::raw(fit("Earnings show up here as your factory sells.", width)),
            Line::raw(""),
            Line::styled(fit("Sales are charted by the second, minute, hour and day.", width), dim),
        ];
        return Page { lines, hits: Vec::new() };
    }
    let mut page = Page { lines: Vec::new(), hits: Vec::new() };
    if width < 16 {
        page.lines.push(Line::styled(fit("Too narrow for the history.", width), dim));
        return page;
    }
    let range = app.hist_range;
    let research = app.hist_research;
    // Pills for the range.
    let short = width < 36;
    let mut spans: Vec<Span> = Vec::new();
    let mut col = 0;
    for (i, r) in Range::ALL.iter().enumerate() {
        let name = if short { ["Sec", "Min", "Hrs", "Day"][i] } else { r.name() };
        let label = format!(" {name} ");
        let st = if *r == range { Style::new().fg(INK).bg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::new().fg(Color::Gray) };
        page.hits.push((0, col, label.chars().count(), StatsHit::Range(*r)));
        col += label.chars().count() + 1;
        spans.push(Span::styled(label, st));
        spans.push(Span::raw(" "));
    }
    page.lines.push(Line::from(spans));

    // The chart: money and research stacked when the pane is tall, else the one `r` picked.
    let pts = h.series(range, now, off);
    let ch = chart_rows(height);
    let stacked = stacked(height);
    for research in if stacked { vec![false, true] } else { vec![research] } {
        let (chart_lines, summary) = chart(&pts, range, research, width, ch);
        page.lines.push(summary);
        page.lines.extend(chart_lines);
        if stacked && !research {
            page.lines.push(Line::raw(""));
        }
    }
    page
}

/// The time to draw at: the app's fixed clock in tests, otherwise now (seconds, zone offset).
fn clock(app: &App) -> (u64, i64) {
    app.clock.unwrap_or_else(|| {
        let now = now_secs();
        (now, hist::local_offset(now))
    })
}

/// The sidebar's compact chart: a title and `ch` rows of bars with the time axis under them,
/// `width` columns wide, over the range in the settings. Shows the most recent bars that fit.
pub(super) fn sidebar_chart(app: &App, research: bool, width: usize, ch: usize) -> (String, Vec<Line<'static>>) {
    let (now, off) = clock(app);
    let range = app.settings.sidebar_graph_range.range();
    let pts = app.world.history.series(range, now, off);
    let fr = frame(&pts, range, research, width);
    let shown = &pts[pts.len() - fr.n..];
    let unit = match range {
        Range::Seconds => "sec",
        Range::Minutes => "min",
        Range::Hours => "h",
        Range::Days => "days",
    };
    let name = if research { "Research" } else { "Earnings" };
    (format!("{name} · last {} {unit}", fr.n), bars(shown, range, research, &fr, ch))
}

/// A bar's value: money or research.
fn val(p: &Point, research: bool) -> f64 {
    if research {
        p.research
    } else {
        p.money
    }
}

/// A round number at or above `v` whose half is also round: 2, 4, 6, 8 or 10 times a power of ten.
fn nice(v: f64) -> f64 {
    if v <= 0.0 {
        return 2.0;
    }
    let e = v.log10().floor();
    let base = 10f64.powf(e);
    let f = v / base;
    (f / 2.0 - 1e-9).ceil().max(1.0) * 2.0 * base
}

/// A research amount for an axis: 0.4, 12, 1.5k.
fn axis_num(v: f64) -> String {
    let s = if v >= 10_000.0 {
        format!("{:.0}k", v / 1e3)
    } else if v >= 1000.0 {
        format!("{:.1}k", v / 1e3)
    } else if v >= 10.0 {
        format!("{v:.0}")
    } else if v >= 1.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    };
    // 1.0k -> 1k, 2.50 -> 2.5, 0.40 -> 0.4
    let (num, unit) = match s.strip_suffix('k') {
        Some(n) => (n.to_string(), "k"),
        None => (s, ""),
    };
    let num = if num.contains('.') { num.trim_end_matches('0').trim_end_matches('.').to_string() } else { num };
    format!("{}{unit}", if num.is_empty() { "0".to_string() } else { num })
}

fn axis_label(v: f64, research: bool) -> String {
    if research {
        axis_num(v)
    } else {
        short_money(v)
    }
}

/// "14:05" for a minute number on the local clock.
fn clock_text(minute: u64) -> String {
    format!("{:02}:{:02}", minute / 60 % 24, minute % 60)
}

/// "Nov 11".
fn date_text(day: u64) -> String {
    let (_, m, d) = civil(day);
    format!("{} {d}", MONTHS[m as usize - 1])
}

/// What a bar covers, for the "best" note.
fn when(range: Range, t: u64) -> String {
    match range {
        Range::Seconds => format!("{}:{:02}", clock_text(t / 60), t % 60),
        Range::Minutes => clock_text(t),
        Range::Hours => format!("{:02}:00 {}", t % 24, date_text(t / 24)),
        Range::Days => date_text(t),
    }
}

/// How the bars sit in a width: the axis scale and label width, the columns left for bars, and how
/// many bars that holds (the most recent ones if not all).
#[derive(Clone, Copy)]
struct Frame {
    top: f64,
    lw: usize,
    avail: usize,
    n: usize,
}

fn frame(pts: &[Point], range: Range, research: bool, width: usize) -> Frame {
    let max_v = pts.iter().map(|p| val(p, research)).fold(0.0, f64::max);
    let top = nice(max_v);
    let lw = [top, top / 2.0, 0.0].iter().map(|v| axis_label(*v, research).chars().count()).max().unwrap_or(2);
    let avail = width.saturating_sub(lw + 1);
    let n = range.count().min(avail.max(1));
    Frame { top, lw, avail, n }
}

/// The summary line and the chart rows (labelled y axis, bars, time axis).
fn chart(pts: &[Point], range: Range, research: bool, width: usize, ch: usize) -> (Vec<Line<'static>>, Line<'static>) {
    let colour = if research { RESEARCH } else { Color::Green };
    let fr = frame(pts, range, research, width);
    let shown = &pts[pts.len() - fr.n..];
    let n = fr.n;

    // Summary: what's shown, the total and the best bar.
    let total: f64 = shown.iter().map(|p| val(p, research)).sum();
    let best = shown.iter().filter(|p| val(p, research) > 0.0).max_by(|a, b| val(a, research).total_cmp(&val(b, research)));
    let name = if research { "Research" } else { "Money" };
    // The most that fits: what's shown, the total and the best bar, dropping details as it narrows.
    let amount = |v: f64, long: bool| {
        if research {
            let n = axis_num((v * 10.0).round() / 10.0);
            if long {
                format!("{n} research")
            } else {
                n
            }
        } else {
            signed_money(v)
        }
    };
    let best_of = |long: bool, with_when: bool| {
        best.map(|b| {
            let when = if with_when { format!(" ({})", when(range, b.t)) } else { String::new() };
            format!(" · best {}{when}", amount(val(b, research), long))
        })
        .unwrap_or_default()
    };
    let span = match range {
        Range::Seconds => format!("last {n} seconds, {}s average", hist::SMOOTH_SECS),
        _ => format!("last {n} {}s", range.unit()),
    };
    let candidates = [
        format!("{name} · {span} · total {}{}", amount(total, true), best_of(true, true)),
        format!("{name} · {span} · total {}{}", amount(total, false), best_of(false, false)),
        format!("{name} · total {}{}", amount(total, false), best_of(false, false)),
        format!("{name} · total {}", amount(total, false)),
    ];
    let text = candidates.iter().find(|c| c.chars().count() <= width).cloned().unwrap_or_else(|| fit(&candidates[3], width));
    let (head, rest) = text.split_at(name.len().min(text.len()));
    let summary =
        Line::from(vec![Span::styled(head.to_string(), Style::new().fg(colour).add_modifier(Modifier::BOLD)), Span::raw(rest.to_string())]);

    (bars(shown, range, research, &fr, ch), summary)
}

/// The painted bars with the y axis beside them and the time axis under them: `ch` rows of bars.
fn bars(shown: &[Point], range: Range, research: bool, fr: &Frame, ch: usize) -> Vec<Line<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let colour = if research { RESEARCH } else { Color::Green };
    let Frame { top, lw, avail, n } = *fr;
    // Spread the bars over the whole width: bar i covers columns xs[i]..xs[i + 1]. With two or
    // more columns a bar, the last one is a gap; with fewer, the bars touch.
    let xs: Vec<usize> = (0..=n).map(|i| i * avail.max(n) / n.max(1)).collect();
    // A gap only once a bar has three columns: at two, half the chart would be gaps (a barcode).
    let gaps = avail >= n * 3;
    let mut lines = Vec::new();
    let rows = ch * 8;
    for r in 0..ch {
        let label = if r == 0 {
            axis_label(top, research)
        } else if r == ch - 1 {
            axis_label(0.0, research)
        } else if r == ch / 2 && ch >= 5 {
            axis_label(top / 2.0, research)
        } else {
            String::new()
        };
        let mut spans = vec![Span::styled(format!("{label:>lw$}┤"), dim)];
        let from_bottom = ch - 1 - r;
        for (i, p) in shown.iter().enumerate() {
            let v = val(p, research);
            let eighths = if v <= 0.0 { 0 } else { ((v / top * rows as f64).round() as usize).clamp(1, rows) };
            let fill = eighths.saturating_sub(from_bottom * 8).min(8);
            let (c, st) = if fill == 8 {
                // A full cell is painted, not drawn: terminals leave thin seams between block
                // glyphs, but a background fills the whole cell.
                (' ', Style::new().bg(colour))
            } else if fill > 0 {
                (BARS[fill - 1], Style::new().fg(colour))
            } else if v <= 0.0 && from_bottom == 0 {
                ('▁', Style::new().fg(FAINT))
            } else {
                (' ', Style::new())
            };
            let w = xs[i + 1] - xs[i];
            let solid = if gaps { w - 1 } else { w };
            spans.push(Span::styled(c.to_string().repeat(solid), st));
            if gaps {
                spans.push(Span::raw(" "));
            }
        }
        lines.push(Line::from(spans));
    }
    lines.push(time_axis(shown, range, lw, &xs));
    lines
}

/// The labels under the bars: how long ago, or the months.
fn time_axis(shown: &[Point], range: Range, lw: usize, xs: &[usize]) -> Line<'static> {
    let dim = Style::new().fg(Color::DarkGray);
    let n = shown.len();
    let w = xs[n];
    let mut labels: Vec<(usize, String)> = Vec::new();
    // "now" is always at the right end, so nothing else may run into it.
    let right = w.saturating_sub(3);
    for (i, p) in shown.iter().enumerate() {
        let age = n - 1 - i;
        let text = match range {
            Range::Seconds if age.is_multiple_of(10) && age > 0 => Some(format!("-{age}s")),
            Range::Minutes if age.is_multiple_of(10) && age > 0 => Some(format!("-{age}m")),
            Range::Hours if age.is_multiple_of(6) && age > 0 => Some(format!("-{age}h")),
            Range::Days => {
                let (_, m, d) = civil(p.t);
                match d {
                    1 => Some(MONTHS[m as usize - 1].to_string()),
                    8 | 15 | 22 => Some(d.to_string()),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(t) = text {
            labels.push((xs[i], t));
        }
    }
    labels.push((right, "now".to_string()));
    // Place from the right, so "now" wins and nothing runs into the label after it.
    let mut row: Vec<char> = vec![' '; w + 1];
    let mut next_start = usize::MAX;
    for (c, t) in labels.into_iter().rev() {
        let end = c + t.chars().count();
        if end <= w && end < next_start {
            for (k, ch) in t.chars().enumerate() {
                row[c + k] = ch;
            }
            next_start = c;
        }
    }
    let text: String = row.into_iter().collect();
    Line::styled(format!("{}{}", " ".repeat(lw + 1), text.trim_end()), dim)
}
