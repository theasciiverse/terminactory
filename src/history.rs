//! Earnings history: money and research earned per second (last minute, this session only),
//! per minute (last hour), per hour (last two days) and per calendar day (last 53 weeks), kept
//! in the save for the History tab.
//!
//! Nothing here draws. Buckets are keyed by *local* time (the clock plus the zone's offset at
//! that moment), so a "day" is the player's own day. Only buckets that were touched are
//! stored, which also tells the heatmap which days were played at all.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// Second buckets kept: the last minute.
pub const SECONDS: u64 = 60;
/// Minute buckets kept: the last hour.
pub const MINUTES: u64 = 60;
/// Hour buckets kept: the last two days.
pub const HOURS: u64 = 48;
/// Day buckets kept: 53 whole weeks, the most a heatmap can show.
pub const DAYS: u64 = 371;
/// Weeks the heatmap can show (its columns).
pub const WEEKS: u64 = 53;

/// How much time each bar of the earnings chart covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    Seconds,
    Minutes,
    Hours,
    Days,
}

impl Range {
    pub const ALL: [Range; 4] = [Range::Seconds, Range::Minutes, Range::Hours, Range::Days];

    pub fn name(self) -> &'static str {
        match self {
            Range::Seconds => "Seconds",
            Range::Minutes => "Minutes",
            Range::Hours => "Hours",
            Range::Days => "Days",
        }
    }
    /// Bars in the whole range.
    pub fn count(self) -> usize {
        match self {
            Range::Seconds => SECONDS as usize,
            Range::Minutes => MINUTES as usize,
            Range::Hours => HOURS as usize,
            Range::Days => DAYS as usize,
        }
    }
    /// "second", "minute", "hour", "day".
    pub fn unit(self) -> &'static str {
        match self {
            Range::Seconds => "second",
            Range::Minutes => "minute",
            Range::Hours => "hour",
            Range::Days => "day",
        }
    }
    pub fn next(self) -> Range {
        Range::ALL[(self as usize + 1).min(3)]
    }
    pub fn prev(self) -> Range {
        Range::ALL[(self as usize).saturating_sub(1)]
    }
}

/// What was earned in one stretch of time. `t` is the bucket's key: minutes, hours or days
/// since 1970-01-01 00:00 local time.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bucket {
    pub t: u64,
    pub money: f64,
    pub research: f64,
}

/// One bar of a series, with gaps filled in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// The bucket key: seconds, minutes, hours or days since 1970, local time.
    pub t: u64,
    pub money: f64,
    pub research: f64,
    /// The game ran in this stretch (even if nothing sold).
    pub seen: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    /// The last minute, second by second. Not saved: it only means something while playing.
    #[serde(skip)]
    pub seconds: VecDeque<Bucket>,
    #[serde(default)]
    pub minutes: VecDeque<Bucket>,
    #[serde(default)]
    pub hours: VecDeque<Bucket>,
    #[serde(default)]
    pub days: VecDeque<Bucket>,
    /// This session only: the lifetime totals (company deliveries left out) at the last sample.
    #[serde(skip)]
    base: Option<(f64, f64)>,
}

/// Seconds east of UTC that the system's time zone is at `now` (0 if it can't be told).
#[cfg(unix)]
pub fn local_offset(now: u64) -> i64 {
    // SAFETY: localtime_r only writes into the `tm` we hand it, and reads the time we point to.
    unsafe {
        let t = now as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return 0;
        }
        tm.tm_gmtoff as i64
    }
}

#[cfg(not(unix))]
pub fn local_offset(_now: u64) -> i64 {
    0
}

/// Seconds on the local clock (never before 1970).
fn local(now: u64, off: i64) -> u64 {
    (now as i64).saturating_add(off).max(0) as u64
}

/// The day number a local second falls on.
pub fn day_of(now: u64, off: i64) -> u64 {
    local(now, off) / 86_400
}

/// Adds to the bucket for `t`, keeping the list in time order.
fn upsert(q: &mut VecDeque<Bucket>, t: u64, money: f64, research: f64) {
    let i = q.partition_point(|b| b.t < t);
    match q.get_mut(i) {
        Some(b) if b.t == t => {
            b.money += money;
            b.research += research;
        }
        _ => q.insert(i, Bucket { t, money, research }),
    }
}

/// Drops buckets that fell out of the last `keep` (counting `newest` as the latest).
fn prune(q: &mut VecDeque<Bucket>, newest: u64, keep: u64) {
    while q.front().is_some_and(|b| b.t + keep <= newest) {
        q.pop_front();
    }
}

impl History {
    /// Once a second: `total` is the lifetime (money, research) earned, without company
    /// deliveries. The first sample of a session only sets the starting point.
    pub fn record(&mut self, now: u64, off: i64, total: (f64, f64)) {
        let (dm, dr) = match self.base.replace(total) {
            Some(b) => (total.0 - b.0, total.1 - b.1),
            None => (0.0, 0.0),
        };
        self.add(local(now, off), dm, dr, true);
    }

    /// Earnings that came in while the game was closed, applied at `now`. They count toward
    /// the hour and the day they were applied in, but not the minute: they were not earned in it.
    pub fn record_offline(&mut self, now: u64, off: i64, money: f64, research: f64) {
        if let Some(b) = self.base.as_mut() {
            b.0 += money;
            b.1 += research;
        }
        self.add(local(now, off), money, research, false);
    }

    fn add(&mut self, ls: u64, money: f64, research: f64, minute: bool) {
        if minute {
            upsert(&mut self.seconds, ls, money, research);
            upsert(&mut self.minutes, ls / 60, money, research);
        }
        prune(&mut self.seconds, ls, SECONDS);
        upsert(&mut self.hours, ls / 3600, money, research);
        upsert(&mut self.days, ls / 86_400, money, research);
        prune(&mut self.minutes, ls / 60, MINUTES);
        prune(&mut self.hours, ls / 3600, HOURS);
        prune(&mut self.days, ls / 86_400, DAYS);
    }

    /// True until anything has been recorded.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.days.is_empty()
    }

    /// The last `range.count()` bars up to now, oldest first, with gaps filled in.
    /// A bar's money is never negative (waste fees can make a stretch's net dip below zero).
    pub fn series(&self, range: Range, now: u64, off: i64) -> Vec<Point> {
        let ls = local(now, off);
        let fill = |q: &VecDeque<Bucket>, newest: u64, n: u64| -> Vec<Point> {
            (0..n)
                .map(|i| {
                    let t = (newest + 1).saturating_sub(n) + i;
                    match q.iter().find(|b| b.t == t) {
                        Some(b) => Point { t, money: b.money.max(0.0), research: b.research.max(0.0), seen: true },
                        None => Point { t, money: 0.0, research: 0.0, seen: false },
                    }
                })
                .collect()
        };
        match range {
            Range::Seconds => fill(&self.seconds, ls, SECONDS),
            Range::Minutes => fill(&self.minutes, ls / 60, MINUTES),
            Range::Hours => fill(&self.hours, ls / 3600, HOURS),
            Range::Days => fill(&self.days, ls / 86_400, DAYS),
        }
    }
}

/// Days since 1970-01-01 are counted from a Thursday. Weeks run Sunday to Saturday, as on GitHub.
pub fn week_of(day: u64) -> u64 {
    (day + 4) / 7
}

/// Which day of the week (0 Sunday to 6 Saturday).
pub fn weekday(day: u64) -> u64 {
    (day + 4) % 7
}

/// (year, month 1-12, day 1-31) of a day number.
pub fn civil(day: u64) -> (i64, u32, u32) {
    let z = day as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

pub const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
pub const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

#[cfg(test)]
mod tests;
