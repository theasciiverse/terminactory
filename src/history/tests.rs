use super::*;

/// 2026-11-11 12:00:00 UTC (a Wednesday).
const NOON: u64 = 1_794_398_400;

fn money(h: &History, range: Range, now: u64) -> f64 {
    h.series(range, now, 0).iter().map(|p| p.money).sum()
}

#[test]
fn civil_dates_and_weekdays() {
    assert_eq!(civil(0), (1970, 1, 1));
    assert_eq!(weekday(0), 4, "a Thursday");
    assert_eq!(civil(NOON / 86_400), (2026, 11, 11));
    assert_eq!(WEEKDAYS[weekday(NOON / 86_400) as usize], "Wed");
    // Leap day, and the day after.
    assert_eq!(civil(19_782), (2024, 2, 29));
    assert_eq!(civil(19_783), (2024, 3, 1));
    // Weeks run Sunday to Saturday.
    let sunday = NOON / 86_400 - 3;
    assert_eq!(weekday(sunday), 0);
    assert_eq!(week_of(sunday), week_of(sunday + 6));
    assert_ne!(week_of(sunday), week_of(sunday - 1));
}

#[test]
fn the_zone_offset_is_sane() {
    let off = local_offset(NOON);
    assert!(off.abs() <= 14 * 3600, "{off}");
}

#[test]
fn earnings_land_in_the_second_minute_hour_and_day_they_came_in() {
    let mut h = History::default();
    h.record(NOON, 0, (0.0, 0.0));
    h.record(NOON + 1, 0, (10.0, 1.0));
    h.record(NOON + 2, 0, (15.0, 1.0));
    let s = h.series(Range::Minutes, NOON + 2, 0);
    assert_eq!(s.len(), 60);
    assert_eq!((s[59].money, s[59].research), (15.0, 1.0));
    assert!(s[59].seen && !s[58].seen);
    assert_eq!(money(&h, Range::Hours, NOON), 15.0);
    assert_eq!(money(&h, Range::Days, NOON), 15.0);
    let secs = h.series(Range::Seconds, NOON + 2, 0);
    assert_eq!(secs.len(), 60);
    assert_eq!((secs[58].money, secs[59].money), (10.0, 5.0), "one bar per second");
    assert_eq!(money(&h, Range::Seconds, NOON + 2), 15.0);
}

#[test]
fn a_new_minute_rolls_to_the_next_bar_and_the_hour_and_day_keep_adding() {
    let mut h = History::default();
    let mut total = 0.0;
    h.record(NOON, 0, (0.0, 0.0));
    // $1 a second for 3 minutes and a bit, then 2 hours later and a day later.
    for s in 1..=200 {
        total += 1.0;
        h.record(NOON + s, 0, (total, 0.0));
    }
    let m = h.series(Range::Minutes, NOON + 200, 0);
    // 12:00:00 is the first second of its minute, which then holds 59 earnings (the first sample is the start).
    let last4: Vec<f64> = m[56..].iter().map(|p| p.money).collect();
    assert_eq!(last4, vec![59.0, 60.0, 60.0, 21.0]);
    assert_eq!(money(&h, Range::Hours, NOON + 200), 200.0);
    total += 5.0;
    h.record(NOON + 7200 + 5, 0, (total, 0.0));
    assert_eq!(money(&h, Range::Hours, NOON + 7205), 205.0, "the hours are separate bars");
    let hours = h.series(Range::Hours, NOON + 7205, 0);
    assert_eq!((hours[47].money, hours[45].money), (5.0, 200.0));
    // The next day.
    total += 7.0;
    h.record(NOON + 86_400, 0, (total, 0.0));
    let days = h.series(Range::Days, NOON + 86_400, 0);
    assert_eq!((days[370].money, days[369].money), (7.0, 205.0));
}

#[test]
fn buckets_age_out() {
    let mut h = History::default();
    h.record(NOON, 0, (0.0, 0.0));
    h.record(NOON + 1, 0, (100.0, 0.0));
    // 59 minutes later it is still the last bar of the hour; 60 and it is gone.
    h.record(NOON + 59 * 60, 0, (100.0, 0.0));
    assert_eq!(h.series(Range::Minutes, NOON + 59 * 60, 0)[0].money, 100.0);
    h.record(NOON + 60 * 60, 0, (100.0, 0.0));
    assert_eq!(money(&h, Range::Minutes, NOON + 3600), 0.0);
    assert!(h.minutes.iter().all(|b| b.t * 60 > NOON + 3600 - 3600 - 60));
    assert_eq!(money(&h, Range::Hours, NOON + 3600), 100.0, "but the hour keeps it");
    // 48 hours on, the hour is gone; the day is still there.
    h.record(NOON + 48 * 3600, 0, (100.0, 0.0));
    assert_eq!(money(&h, Range::Hours, NOON + 48 * 3600), 0.0);
    assert_eq!(money(&h, Range::Days, NOON + 48 * 3600), 100.0);
    assert!(h.hours.len() <= HOURS as usize);
    // 371 days on, the day goes, and the memory stays bounded.
    h.record(NOON + 371 * 86_400, 0, (100.0, 0.0));
    assert_eq!(money(&h, Range::Days, NOON + 371 * 86_400), 0.0);
    for d in 0..800 {
        h.record(NOON + d * 86_400, 0, (100.0, 0.0));
    }
    assert!(h.days.len() <= DAYS as usize && h.hours.len() <= HOURS as usize && h.minutes.len() <= MINUTES as usize);
}

#[test]
fn days_follow_the_local_clock() {
    let mut h = History::default();
    // 23:30 UTC on the 11th is already the 12th at +2h, and still the 11th at -5h.
    let t = NOON + 11 * 3600 + 1800;
    h.record(t, 2 * 3600, (0.0, 0.0));
    h.record(t + 1, 2 * 3600, (9.0, 0.0));
    let east = h.series(Range::Days, t + 1, 2 * 3600);
    assert_eq!(civil(east[370].t), (2026, 11, 12));
    assert_eq!(east[370].money, 9.0);
    let west = h.series(Range::Days, t + 1, -5 * 3600);
    assert_eq!(civil(west[370].t), (2026, 11, 11));
    assert_eq!(west[370].money, 0.0, "the bucket is where the clock was when it was earned");
}

#[test]
fn the_first_sample_of_a_session_is_only_a_starting_point() {
    let mut h = History::default();
    // A loaded save: lifetime totals are already large.
    h.record(NOON, 0, (50_000.0, 900.0));
    assert_eq!(money(&h, Range::Days, NOON), 0.0);
    assert!(!h.is_empty(), "but the day counts as played");
    h.record(NOON + 1, 0, (50_003.0, 900.0));
    assert_eq!(money(&h, Range::Days, NOON + 1), 3.0);
}

#[test]
fn offline_pay_counts_for_the_hour_and_day_it_was_applied() {
    let mut h = History::default();
    h.record_offline(NOON, 0, 5000.0, 40.0);
    assert_eq!(money(&h, Range::Minutes, NOON), 0.0, "not earned in that minute");
    assert_eq!(money(&h, Range::Hours, NOON), 5000.0);
    assert_eq!(money(&h, Range::Days, NOON), 5000.0);
    // And it isn't counted again by the next sample.
    h.record(NOON, 0, (5000.0, 40.0));
    h.record(NOON + 1, 0, (5002.0, 40.0));
    assert_eq!(money(&h, Range::Days, NOON + 1), 5002.0);
    // Applied after a session's first sample: the baseline moves too.
    h.record_offline(NOON + 2, 0, 100.0, 0.0);
    h.record(NOON + 3, 0, (5102.0, 40.0));
    assert_eq!(money(&h, Range::Days, NOON + 3), 5102.0);
}

#[test]
fn a_dip_below_zero_shows_as_nothing() {
    let mut h = History::default();
    h.record(NOON, 0, (100.0, 0.0));
    h.record(NOON + 1, 0, (97.0, 0.0)); // a waste fee
    assert_eq!(money(&h, Range::Minutes, NOON + 1), 0.0);
}

#[test]
fn clocks_that_jump_back_keep_the_list_in_order() {
    let mut h = History::default();
    h.record(NOON, 0, (0.0, 0.0));
    h.record(NOON + 7200, 0, (10.0, 0.0));
    h.record(NOON + 3600, 0, (15.0, 0.0));
    let ts: Vec<u64> = h.hours.iter().map(|b| b.t).collect();
    assert!(ts.windows(2).all(|w| w[0] < w[1]), "{ts:?}");
}

#[test]
fn the_tiny_clock_does_not_panic() {
    let h = History::default();
    for r in Range::ALL {
        assert_eq!(h.series(r, 0, -3600).len(), r.count());
    }
}

#[test]
fn ranges_step_and_stop_at_the_ends() {
    assert_eq!(Range::Seconds.prev(), Range::Seconds);
    assert_eq!(Range::Seconds.next(), Range::Minutes);
    assert_eq!(Range::Days.next(), Range::Days);
    assert_eq!(Range::Days.prev(), Range::Hours);
}
