use polymarket_analysis::collector::{BackoffPolicy, ScheduledProvider};

#[test]
fn polymarket_backoff_is_fast_but_capped() {
    let mut schedule = ScheduledProvider::new("polymarket", 1, BackoffPolicy::polymarket());
    assert_eq!(schedule.current_delay_seconds(), 1);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 2);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 5);
    schedule.record_success();
    assert_eq!(schedule.current_delay_seconds(), 1);
}

#[test]
fn oddsportal_backoff_doubles_to_cap() {
    let mut schedule = ScheduledProvider::new("oddsportal", 60, BackoffPolicy::oddsportal(60));
    assert_eq!(schedule.current_delay_seconds(), 60);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 120);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 240);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 300);
}
