// c20_e07_review_blocking_and_concurrency.rs
//
// CONCEPT: code review — this file COMPILES, but has 2 planted bugs typical
//          of chapter 20 (a "sleep" that blocks the whole executor thread
//          instead of yielding it, and three independent awaits run one
//          after another instead of concurrently).
// GUIDE:   chapters/20-async-basics/guide.md  (section "Concurrency: join and join_all")
// DATABROKER: databroker/src/broker.rs — `get_values_broker` builds one
//             future per provider and awaits them with
//             `futures::future::join_all` (commit 748a914 "Query providers
//             concurrently"); before that commit it awaited them one by
//             one, tripling the latency of a `GetValues` call.
// EXPECT:  tests fail: a task that should still be pending under a paused
//          clock finishes immediately (and eats real wall-clock time doing
//          it), and polling three sensors takes three delays instead of one.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

use std::time::Duration;

/// Confirms a write by waiting `delay`, then returning `true`.
async fn slow_confirm(delay: Duration) -> bool {
    std::thread::sleep(delay);
    true
}

/// Reads one sensor after waiting `delay` for it to respond.
async fn read_sensor(id: u32, delay: Duration) -> u32 {
    tokio::time::sleep(delay).await;
    id * 10
}

/// Polls three sensors and returns their readings in order.
async fn poll_three_sensors(delay: Duration) -> Vec<u32> {
    let a = read_sensor(1, delay).await;
    let b = read_sensor(2, delay).await;
    let c = read_sensor(3, delay).await;
    vec![a, b, c]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn slow_confirm_does_not_block_the_executor() {
        let start = std::time::Instant::now();
        let handle = tokio::spawn(slow_confirm(Duration::from_millis(20)));
        tokio::task::yield_now().await;
        // The clock is paused and never advanced, so a real `.await` on a
        // tokio timer must still be pending — and getting there must not
        // have cost any real wall-clock time.
        assert!(start.elapsed() < Duration::from_millis(10));
        assert!(!handle.is_finished());
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn sensors_are_polled_concurrently() {
        let delay = Duration::from_millis(100);
        let start = tokio::time::Instant::now();
        // With the clock paused, tokio auto-advances virtual time to the
        // next scheduled timer whenever nothing else can make progress —
        // so the elapsed virtual time here is exactly how much waiting
        // this call actually needed.
        let result = poll_three_sensors(delay).await;
        assert_eq!(result, vec![10, 20, 30]);
        assert_eq!(
            start.elapsed(),
            delay,
            "polling three independent sensors concurrently should take exactly one delay, not three"
        );
    }
}
