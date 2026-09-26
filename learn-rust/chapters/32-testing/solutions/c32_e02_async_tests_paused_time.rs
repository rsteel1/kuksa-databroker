// c32_e02_async_tests_paused_time.rs
//
// CONCEPT: deterministic async tests — `#[tokio::test(start_paused = true)]` and virtual time.
// GUIDE:   chapters/32-testing/guide.md  (section "Async tests and virtual time")
// DATABROKER: databroker/src/broker.rs — `ChangeSubscription::notify` (skips when
//             `last_emitted.elapsed() < duration`) and
//             `test_subscribe_initial_notification_delivered_with_min_sample_interval`
// EXPECT:  tests fail: `todo!()` panics
//
// Subscribers may ask for a *minimum sample interval*: "at most one update
// every 100 ms". Testing that with real sleeps is slow and flaky (CI machines
// stall). With `start_paused = true` tokio's clock is virtual: time only
// moves when every task is idle waiting on a timer, and then it jumps
// straight to the next timer. A "10 second" test takes microseconds, and
// the timestamps are exact.
//
// YOUR TASK
//   1. Implement `sample`: receive values until the channel closes; keep a
//      value if it is the first one, or if at least `interval` has passed
//      since the last KEPT value. Record (ms since `sample` started, value).
//      Use `tokio::time::Instant` (it follows the virtual clock; std's doesn't).
//   2. Write the body of the last test (instructions above it).
//   Do not edit the other tests.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

async fn sample(mut rx: mpsc::Receiver<f32>, interval: Duration) -> Vec<(u128, f32)> {
    let start = Instant::now();
    let mut last_emitted: Option<Instant> = None;
    let mut kept = Vec::new();
    while let Some(value) = rx.recv().await {
        let now = Instant::now();
        let due = last_emitted.is_none_or(|last| now.duration_since(last) >= interval);
        if due {
            kept.push((now.duration_since(start).as_millis(), value));
            last_emitted = Some(now);
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spawns a producer: for each (delay_ms, value), sleep then send.
    /// The channel closes when the producer finishes.
    fn produce(script: Vec<(u64, f32)>) -> mpsc::Receiver<f32> {
        let (tx, rx) = mpsc::channel(16);
        tokio::spawn(async move {
            for (delay_ms, value) in script {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                tx.send(value).await.expect("receiver alive");
            }
        });
        rx
    }

    #[tokio::test(start_paused = true)]
    async fn drops_values_that_come_too_fast() {
        // t=0: 1.0, t=30: 2.0, t=60: 3.0, t=110: 4.0
        let rx = produce(vec![(0, 1.0), (30, 2.0), (30, 3.0), (50, 4.0)]);
        let kept = sample(rx, Duration::from_millis(100)).await;
        assert_eq!(kept, vec![(0, 1.0), (110, 4.0)]);
    }

    #[tokio::test(start_paused = true)]
    async fn virtual_time_makes_long_tests_instant() {
        let real_start = std::time::Instant::now();
        // An hour of virtual time, one value per minute.
        let rx = produce((0..60).map(|i| (60_000, i as f32)).collect());
        let kept = sample(rx, Duration::from_secs(120)).await;
        assert_eq!(kept.len(), 30, "every second value is kept");
        assert_eq!(kept[1], (180_000, 2.0));
        assert!(
            real_start.elapsed() < Duration::from_secs(2),
            "no real waiting"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn empty_channel_gives_empty_result() {
        let rx = produce(vec![]);
        assert!(sample(rx, Duration::from_millis(1)).await.is_empty());
    }

    // YOUR TEST: the rule is ">= interval", not "> interval". Prove it:
    // produce three values exactly 100 ms apart (t=0, 100, 200), sample with a
    // 100 ms interval, and assert that all three are kept with their times.
    #[tokio::test(start_paused = true)]
    async fn values_exactly_one_interval_apart_are_all_kept() {
        let rx = produce(vec![(0, 1.0), (100, 2.0), (100, 3.0)]);
        let kept = sample(rx, Duration::from_millis(100)).await;
        assert_eq!(kept, vec![(0, 1.0), (100, 2.0), (200, 3.0)]);
    }
}
