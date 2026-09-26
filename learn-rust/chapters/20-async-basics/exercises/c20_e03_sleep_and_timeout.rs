// c20_e03_sleep_and_timeout.rs
//
// CONCEPT: `tokio::time::sleep` and `tokio::time::timeout`; testing time
//          with a paused clock.
// GUIDE:   chapters/20-async-basics/guide.md  (section "Time: sleep, timeout and paused clocks")
// DATABROKER: databroker/src/broker.rs — `update_filter_to_providers` wraps
//             `signal_provider.update_filter(...)` in
//             `tokio::time::timeout(PROVIDER_UPDATE_FILTER_TIMEOUT, ...)`
// EXPECT:  tests fail: `todo!()` panics
//
// `timeout(duration, future)` returns `Ok(value)` if the future finished in
// time, or `Err(Elapsed)` if not — and in that case the inner future is
// DROPPED (cancelled). That is how the databroker makes sure one hung
// provider cannot stall the housekeeping task forever.
//
// The tests use `#[tokio::test(start_paused = true)]`: Tokio's clock is
// frozen and jumps forward instantly whenever every task is waiting on a
// timer. A "5 second" test finishes in microseconds and is 100% repeatable.
//
// YOUR TASK
//   Implement `ask_provider` and `ask_provider_with_limit`.
//   Do not edit the tests.

// I AM NOT DONE

use std::time::Duration;
use tokio::time::{sleep, timeout};

#[derive(Debug, PartialEq)]
enum AskError {
    TimedOut,
}

/// A provider that needs `delay` before it answers with `value`.
/// Use `sleep` (never `std::thread::sleep` inside async code!).
async fn ask_provider(value: i32, delay: Duration) -> i32 {
    todo!()
}

/// Like `ask_provider`, but gives up after `limit`.
async fn ask_provider_with_limit(
    value: i32,
    delay: Duration,
    limit: Duration,
) -> Result<i32, AskError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;

    #[tokio::test(start_paused = true)]
    async fn provider_answers_after_delay() {
        let start = Instant::now();
        assert_eq!(ask_provider(7, Duration::from_millis(250)).await, 7);
        assert_eq!(start.elapsed(), Duration::from_millis(250));
    }

    #[tokio::test(start_paused = true)]
    async fn fast_provider_is_ok() {
        let result =
            ask_provider_with_limit(42, Duration::from_millis(100), Duration::from_secs(1)).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test(start_paused = true)]
    async fn hung_provider_times_out_at_the_limit() {
        let start = Instant::now();
        let result =
            ask_provider_with_limit(42, Duration::from_secs(3600), Duration::from_secs(1)).await;
        assert_eq!(result, Err(AskError::TimedOut));
        // We waited exactly the limit, not the hour.
        assert_eq!(start.elapsed(), Duration::from_secs(1));
    }
}
