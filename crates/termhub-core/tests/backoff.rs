use std::time::Duration;

use termhub_core::backoff::Backoff;

#[test]
fn first_two_attempts_within_one_second_use_one_second() {
    let mut b = Backoff::default();
    assert_eq!(b.next_delay(), Duration::from_secs(1));
    assert_eq!(b.next_delay(), Duration::from_secs(1));
}

#[test]
fn after_two_one_seconds_starts_exponential() {
    let mut b = Backoff::default();
    let _ = b.next_delay();
    let _ = b.next_delay();
    assert_eq!(b.next_delay(), Duration::from_secs(2));
    assert_eq!(b.next_delay(), Duration::from_secs(4));
    assert_eq!(b.next_delay(), Duration::from_secs(8));
    assert_eq!(b.next_delay(), Duration::from_secs(16));
    assert_eq!(b.next_delay(), Duration::from_secs(30));
    assert_eq!(b.next_delay(), Duration::from_secs(30));
}

#[test]
fn reset_starts_over() {
    let mut b = Backoff::default();
    for _ in 0..5 {
        let _ = b.next_delay();
    }
    b.reset();
    assert_eq!(b.next_delay(), Duration::from_secs(1));
}

#[test]
fn attempt_counter_increments() {
    let mut b = Backoff::default();
    assert_eq!(b.attempt(), 0);
    let _ = b.next_delay();
    assert_eq!(b.attempt(), 1);
    let _ = b.next_delay();
    assert_eq!(b.attempt(), 2);
}
