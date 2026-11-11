//! Earnings while the game was closed: when they apply, and how much.

use super::*;

/// A world that earned $2 a second and sold a copper plate a second, saved at t = 1000.
fn earner() -> World {
    let mut w = World::new();
    w.rate = (2.0, 0.0);
    w.rate_long = (2.0, 0.0);
    w.sale_rate = vec![0.0; ITEMS];
    w.sale_rate[Item::CopperPlate.idx()] = 1.0;
    w.saved_at = 1_000;
    w
}

#[test]
fn no_pay_without_a_save_time_or_for_a_clock_that_went_backwards() {
    let mut w = earner();
    w.saved_at = 0;
    assert!(w.apply_offline(100_000).is_none(), "a world that was never saved");
    let mut w = earner();
    assert!(w.apply_offline(1_000).is_none(), "same instant");
    assert!(w.apply_offline(500).is_none(), "clock set back");
    assert_eq!(w.econ.money, START_MONEY);
}

#[test]
fn a_minute_away_is_the_least_that_pays() {
    let mut w = earner();
    assert!(w.apply_offline(1_059).is_none());
    assert_eq!(w.econ.money, START_MONEY);
    let (away, m, _) = w.apply_offline(1_060).unwrap();
    assert_eq!((away, m), (60, 120.0));
}

#[test]
fn saves_without_per_item_rates_count_ten_minutes_of_research() {
    let mut w = earner();
    w.sale_rate.clear(); // saved before per-item rates were kept
    w.rate = (0.0, 3.0);
    w.rate_long = (0.0, 0.0);
    let (_, m, r) = w.apply_offline(1_000 + 300).unwrap();
    assert_eq!((m, r), (0.0, 900.0), "five minutes away: the whole time at the last rate");
    let mut w = earner();
    w.sale_rate.clear();
    w.rate = (0.0, 3.0);
    w.rate_long = (0.0, 0.0);
    let (_, _, r) = w.apply_offline(1_000 + 10 * 3600).unwrap();
    assert_eq!(r, 1_800.0, "but never more than ten minutes' worth");
}

#[test]
fn research_follows_what_the_trade_posts_actually_earned_clamped_to_between_none_and_double() {
    let plate = Item::CopperPlate;
    let full = |w: &World| w.econ.next_research(plate);
    let away = |share_of_full: f64| {
        let mut w = earner();
        w.rate = (0.0, share_of_full * full(&w));
        w.rate_long = (0.0, 0.0);
        w.apply_offline(1_000 + 600).map(|(_, _, r)| r)
    };
    let one = away(1.0).unwrap();
    assert!(one > 0.0);
    // Earning less than full price (posts on a split): proportionally less.
    assert!((away(0.5).unwrap() - one * 0.5).abs() < 1e-6);
    // Earning far more than the curve says is capped at twice.
    assert!((away(50.0).unwrap() - one * 2.0).abs() < 1e-6);
    // Earning no research at all: nothing, and nothing to report.
    assert_eq!(away(0.0), None);
}

#[test]
fn a_negative_rate_pays_nothing_instead_of_a_debt() {
    let mut w = earner();
    w.rate = (-3.0, 0.0); // waste fees outran sales
    w.rate_long = (-1.0, 0.0);
    w.sale_rate.clear();
    assert!(w.apply_offline(1_000 + 3600).is_none());
    assert_eq!(w.econ.money, START_MONEY, "no money taken");
    // With research owed, the money part is still just zero.
    let mut w = earner();
    w.rate = (-3.0, 2.0);
    w.rate_long = (-1.0, 0.0);
    w.sale_rate.clear();
    let (_, m, r) = w.apply_offline(1_000 + 600).unwrap();
    assert_eq!((m, r), (0.0, 1200.0));
    assert_eq!(w.econ.money, START_MONEY);
}

#[test]
fn applying_twice_pays_twice() {
    // Pinned: apply_offline doesn't move `saved_at`, so a second call (a second load of the same
    // file, with no save in between) pays for the same time again. `save` is what resets it.
    let mut w = earner();
    let (_, first, _) = w.apply_offline(1_000 + 600).unwrap();
    let (_, second, _) = w.apply_offline(1_000 + 600).unwrap();
    assert_eq!((first, second), (1_200.0, 1_200.0));
    assert_eq!(w.econ.money, START_MONEY + 2_400.0);
    let path = std::env::temp_dir().join(format!("factory-test-offline-{}.json", std::process::id()));
    w.save(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(w.saved_at > 1_000, "saving stamps the time");
}
