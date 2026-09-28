// c23_e06_review_concurrent_and_bounded.rs
//
// CONCEPT: code review — this file COMPILES, but has 2 planted bugs typical
//          of chapter 23 (providers queried one after another instead of
//          concurrently, and a semaphore permit released before the call it
//          was supposed to guard even starts, so the concurrency limit does
//          nothing).
// GUIDE:   chapters/23-async-traits-and-providers/guide.md  (section "GetValues: concurrent, bounded, with fallback")
// DATABROKER: databroker/src/broker.rs — `AuthorizedAccess::get_values_broker`
//             queries every provider concurrently and bounds the fan-out;
//             querying providers one at a time, or bounding it with a
//             permit that's dropped too early, both defeat the point.
// EXPECT:  tests fail: querying providers takes as long as the sum of their
//          delays instead of the slowest one, and more calls run at once
//          than the configured limit allows.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

use futures::future::join_all;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

#[async_trait::async_trait]
trait SignalProvider: Send + Sync {
    async fn value(&self) -> f64;
}

struct SlowProvider {
    id: i32,
    delay: Duration,
    in_flight: Arc<AtomicUsize>,
    max_in_flight: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl SignalProvider for SlowProvider {
    async fn value(&self) -> f64 {
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_in_flight.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.id as f64
    }
}

/// Queries every provider and returns their values, in order.
async fn query_all_concurrently(providers: &[Arc<dyn SignalProvider>]) -> Vec<f64> {
    let mut results = Vec::new();
    for provider in providers {
        results.push(provider.value().await);
    }
    results
}

/// Queries every provider, never running more than `limit` calls at once.
async fn bounded_run(providers: &[Arc<dyn SignalProvider>], limit: usize) -> Vec<f64> {
    let semaphore = Arc::new(Semaphore::new(limit));
    let futures = providers.iter().cloned().map(|provider| {
        let semaphore = semaphore.clone();
        async move {
            let permit = semaphore.acquire().await.expect("semaphore not closed");
            drop(permit);
            provider.value().await
        }
    });
    join_all(futures).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_providers(n: i32, delay: Duration) -> (Vec<Arc<dyn SignalProvider>>, Arc<AtomicUsize>) {
        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_in_flight = Arc::new(AtomicUsize::new(0));
        let providers = (0..n)
            .map(|id| {
                Arc::new(SlowProvider {
                    id,
                    delay,
                    in_flight: in_flight.clone(),
                    max_in_flight: max_in_flight.clone(),
                }) as Arc<dyn SignalProvider>
            })
            .collect();
        (providers, max_in_flight)
    }

    #[tokio::test(start_paused = true)]
    async fn providers_are_queried_concurrently() {
        let (providers, _) = make_providers(5, Duration::from_millis(100));
        let start = tokio::time::Instant::now();
        let results = query_all_concurrently(&providers).await;
        assert_eq!(results, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            start.elapsed(),
            Duration::from_millis(100),
            "querying 5 independent providers concurrently should take one delay, not five"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_run_never_exceeds_the_limit() {
        let (providers, max_in_flight) = make_providers(6, Duration::from_millis(50));
        let results = bounded_run(&providers, 2).await;
        assert_eq!(results.len(), 6);
        assert!(
            max_in_flight.load(Ordering::SeqCst) <= 2,
            "at most 2 calls should ever be in flight at once, saw {}",
            max_in_flight.load(Ordering::SeqCst)
        );
    }
}
