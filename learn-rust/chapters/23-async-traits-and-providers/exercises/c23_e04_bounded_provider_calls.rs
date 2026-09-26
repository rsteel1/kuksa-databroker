// c23_e04_bounded_provider_calls.rs
//
// CONCEPT: putting it together — snapshot providers, query them concurrently,
//          bound each call with a timeout, fall back to the database, and
//          keep the caller's order.
// GUIDE:   chapters/23-async-traits-and-providers/guide.md  (section "GetValues: concurrent, bounded, with fallback")
// DATABROKER: databroker/src/broker.rs — `AuthorizedAccess::get_values_broker`
//             (commits e1fdfd1, 748a914 and 7a210d0)
// EXPECT:  tests fail: `todo!()` panics
//
// This is a simplified `get_values_broker`. Rules:
//   1. A provider is only asked about requested signals it provides
//      (skip providers with no overlap entirely).
//   2. All providers are queried CONCURRENTLY; each call is bounded by
//      `PROVIDER_TIMEOUT`.
//   3. Provider error or timeout -> read all its signals from `database`.
//   4. Partial answer -> read the signals it didn't return from `database`.
//   5. Requested signals no provider covers -> read from `database`.
//   6. Any signal that must come from `database` but isn't there ->
//      `Err(NotFound(id))`.
//   7. The result lists every requested id exactly once, in request order.
//
// YOUR TASK
//   Implement `get_values`. Tip: this is the hardest exercise of Part II.
//   Sketch the steps on paper first; the hints walk through them.
//   Do not edit the tests.

// I AM NOT DONE

use futures::future::join_all;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

const PROVIDER_TIMEOUT: Duration = Duration::from_secs(1);

#[async_trait::async_trait]
trait SignalProvider: Send + Sync {
    /// May return only some of the ids (a partial answer), or fail.
    async fn values(&self, ids: Vec<i32>) -> Result<HashMap<i32, f64>, ()>;
}

#[derive(Debug, PartialEq)]
struct NotFound(i32);

async fn get_values(
    providers: &[(Arc<dyn SignalProvider>, HashSet<i32>)],
    database: &HashMap<i32, f64>,
    requested: &[i32],
) -> Result<Vec<(i32, f64)>, NotFound> {
    todo!()
}

// ---- test doubles ---------------------------------------------------------

/// Answers with `answers` (filtered to what was asked) after `delay`,
/// counting how often it was called.
struct FakeProvider {
    answers: HashMap<i32, f64>,
    delay: Duration,
    fail: bool,
    calls: std::sync::atomic::AtomicUsize,
}

impl FakeProvider {
    fn new(answers: &[(i32, f64)], delay: Duration) -> Arc<Self> {
        Arc::new(FakeProvider {
            answers: answers.iter().copied().collect(),
            delay,
            fail: false,
            calls: Default::default(),
        })
    }
    fn failing() -> Arc<Self> {
        Arc::new(FakeProvider {
            answers: HashMap::new(),
            delay: Duration::ZERO,
            fail: true,
            calls: Default::default(),
        })
    }
    fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl SignalProvider for FakeProvider {
    async fn values(&self, ids: Vec<i32>) -> Result<HashMap<i32, f64>, ()> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        if self.fail {
            return Err(());
        }
        Ok(ids
            .into_iter()
            .filter_map(|id| self.answers.get(&id).map(|v| (id, *v)))
            .collect())
    }
}

fn entry(provider: Arc<FakeProvider>, ids: &[i32]) -> (Arc<dyn SignalProvider>, HashSet<i32>) {
    (provider, ids.iter().copied().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;

    fn database() -> HashMap<i32, f64> {
        HashMap::from([(1, 1.0), (2, 2.0), (3, 3.0), (4, 4.0)])
    }

    #[tokio::test(start_paused = true)]
    async fn provider_values_win_and_order_is_kept() {
        let fast = FakeProvider::new(&[(2, 20.0), (3, 30.0)], Duration::from_millis(10));
        let providers = [entry(fast, &[2, 3])];
        let result = get_values(&providers, &database(), &[3, 1, 2]).await;
        assert_eq!(result, Ok(vec![(3, 30.0), (1, 1.0), (2, 20.0)]));
    }

    #[tokio::test(start_paused = true)]
    async fn hung_provider_is_bounded_and_falls_back() {
        let hung = FakeProvider::new(&[(1, 10.0)], Duration::from_secs(3600));
        let fast = FakeProvider::new(&[(2, 20.0)], Duration::from_millis(10));
        let providers = [entry(hung, &[1]), entry(fast, &[2])];
        let start = Instant::now();
        let result = get_values(&providers, &database(), &[1, 2]).await;
        assert_eq!(result, Ok(vec![(1, 1.0), (2, 20.0)]));
        assert_eq!(start.elapsed(), PROVIDER_TIMEOUT);
    }

    #[tokio::test(start_paused = true)]
    async fn slow_providers_are_queried_concurrently() {
        let a = FakeProvider::new(&[(1, 10.0)], Duration::from_millis(800));
        let b = FakeProvider::new(&[(2, 20.0)], Duration::from_millis(800));
        let providers = [entry(a, &[1]), entry(b, &[2])];
        let start = Instant::now();
        let result = get_values(&providers, &database(), &[1, 2]).await;
        assert_eq!(result, Ok(vec![(1, 10.0), (2, 20.0)]));
        assert_eq!(start.elapsed(), Duration::from_millis(800)); // not 1600
    }

    #[tokio::test(start_paused = true)]
    async fn partial_answer_falls_back_per_signal() {
        let partial = FakeProvider::new(&[(1, 10.0)], Duration::ZERO);
        let providers = [entry(partial, &[1, 2])];
        let result = get_values(&providers, &database(), &[1, 2]).await;
        assert_eq!(result, Ok(vec![(1, 10.0), (2, 2.0)]));
    }

    #[tokio::test(start_paused = true)]
    async fn failing_provider_falls_back() {
        let providers = [entry(FakeProvider::failing(), &[4])];
        let result = get_values(&providers, &database(), &[4]).await;
        assert_eq!(result, Ok(vec![(4, 4.0)]));
    }

    #[tokio::test(start_paused = true)]
    async fn unrelated_providers_are_not_asked() {
        let unrelated = FakeProvider::new(&[(9, 9.0)], Duration::ZERO);
        let providers = [entry(unrelated.clone(), &[9])];
        let result = get_values(&providers, &database(), &[1]).await;
        assert_eq!(result, Ok(vec![(1, 1.0)]));
        assert_eq!(unrelated.calls(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn missing_signal_is_an_error() {
        let result = get_values(&[], &database(), &[1, 42]).await;
        assert_eq!(result, Err(NotFound(42)));
    }
}
