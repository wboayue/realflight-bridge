use super::*;
use std::thread;

#[test]
fn new_starts_with_zero_counts() {
    let engine = StatisticsEngine::new();
    let snapshot = engine.snapshot();

    assert_eq!(snapshot.request_count, 0);
    assert_eq!(snapshot.error_count, 0);
}

#[test]
fn increment_request_count_increases_count() {
    let engine = StatisticsEngine::new();

    engine.increment_request_count();
    engine.increment_request_count();
    engine.increment_request_count();

    assert_eq!(engine.snapshot().request_count, 3);
}

#[test]
fn increment_error_count_increases_count() {
    let engine = StatisticsEngine::new();

    engine.increment_error_count();
    engine.increment_error_count();

    assert_eq!(engine.snapshot().error_count, 2);
}

#[test]
fn runtime_increases_over_time() {
    let engine = StatisticsEngine::new();

    thread::sleep(Duration::from_millis(10));

    let snapshot = engine.snapshot();
    assert!(snapshot.runtime >= Duration::from_millis(10));
}

#[test]
fn frequency_calculated_correctly() {
    let engine = StatisticsEngine::new();

    // Wait a bit then add requests
    thread::sleep(Duration::from_millis(50));
    engine.increment_request_count();
    engine.increment_request_count();

    let snapshot = engine.snapshot();
    // Frequency should be roughly 2 / 0.05 = 40, but allow wide margin
    assert!(snapshot.frequency > 0.0);
    assert!(snapshot.frequency < 100.0);
}

#[test]
fn default_creates_new_engine() {
    let engine = StatisticsEngine::default();
    let snapshot = engine.snapshot();

    assert_eq!(snapshot.request_count, 0);
    assert_eq!(snapshot.error_count, 0);
}
