// c32_e03_test_doubles_for_traits.rs
//
// CONCEPT: test doubles — small fake implementations of a trait that behave badly on purpose.
// GUIDE:   chapters/32-testing/guide.md  (section "Test doubles for trait objects")
// DATABROKER: databroker/src/broker.rs tests — `HangingSignalProvider`, `CountingSignalProvider`,
//             `FailingUpdateFilterProvider`, `PartialResponseProvider`, and
//             `test_get_values_provider_does_not_block_subscriptions`
// EXPECT:  tests fail: `todo!()` panics
//
// `get_values` below talks to providers through `Arc<dyn SignalProvider>`.
// Because it only knows the trait, a test can hand it *anything* that
// implements the trait: a provider that never answers, one that errors, one
// that counts calls. That's how the databroker proves a hung provider can't
// block the broker (commits e1fdfd1 and 748a914).
//
// The code under test is correct. YOUR TASK is to implement the doubles:
//   StaticProvider      answers with its stored values, only for the ids requested
//   FailingProvider     always Err(())
//   HangingProvider     never returns (hint: `std::future::pending()`)
//   CountingProvider    wraps a StaticProvider and counts calls in an AtomicUsize
//   UnavailableProvider is_available() == false; values() must never be called
//                       (use `unreachable!` so a bug in get_values is loud)
//   Do not edit get_values or the tests.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[async_trait]
trait SignalProvider: Send + Sync {
    async fn values(&self, ids: &[i32]) -> Result<HashMap<i32, f32>, ()>;
    fn is_available(&self) -> bool;
}

// ---------------- code under test (do not change) ----------------

/// Ask every available provider concurrently (each bounded by `timeout`),
/// then fall back to `db` for any id no provider answered.
async fn get_values(
    providers: &[Arc<dyn SignalProvider>],
    ids: &[i32],
    db: &HashMap<i32, f32>,
    timeout: Duration,
) -> HashMap<i32, f32> {
    let calls = providers
        .iter()
        .filter(|p| p.is_available())
        .map(|p| tokio::time::timeout(timeout, p.values(ids)));
    let mut result = HashMap::new();
    for answer in futures::future::join_all(calls).await {
        if let Ok(Ok(values)) = answer {
            result.extend(values);
        }
    }
    for id in ids {
        if !result.contains_key(id) {
            if let Some(v) = db.get(id) {
                result.insert(*id, *v);
            }
        }
    }
    result
}

// --------------------------- test doubles ---------------------------

struct StaticProvider {
    values: HashMap<i32, f32>,
}

#[async_trait]
impl SignalProvider for StaticProvider {
    async fn values(&self, ids: &[i32]) -> Result<HashMap<i32, f32>, ()> {
        Ok(ids
            .iter()
            .filter_map(|id| self.values.get(id).map(|v| (*id, *v)))
            .collect())
    }
    fn is_available(&self) -> bool {
        true
    }
}

struct FailingProvider;

#[async_trait]
impl SignalProvider for FailingProvider {
    async fn values(&self, _ids: &[i32]) -> Result<HashMap<i32, f32>, ()> {
        Err(())
    }
    fn is_available(&self) -> bool {
        true
    }
}

struct HangingProvider;

#[async_trait]
impl SignalProvider for HangingProvider {
    async fn values(&self, _ids: &[i32]) -> Result<HashMap<i32, f32>, ()> {
        std::future::pending().await
    }
    fn is_available(&self) -> bool {
        true
    }
}

struct CountingProvider {
    calls: AtomicUsize,
    inner: StaticProvider,
}

#[async_trait]
impl SignalProvider for CountingProvider {
    async fn values(&self, ids: &[i32]) -> Result<HashMap<i32, f32>, ()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.values(ids).await
    }
    fn is_available(&self) -> bool {
        true
    }
}

struct UnavailableProvider;

#[async_trait]
impl SignalProvider for UnavailableProvider {
    async fn values(&self, _ids: &[i32]) -> Result<HashMap<i32, f32>, ()> {
        unreachable!("get_values must skip unavailable providers")
    }
    fn is_available(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(1);

    fn db() -> HashMap<i32, f32> {
        HashMap::from([(1, 10.0), (2, 20.0), (3, 30.0)])
    }

    fn fixed(pairs: &[(i32, f32)]) -> StaticProvider {
        StaticProvider {
            values: pairs.iter().copied().collect(),
        }
    }

    #[tokio::test]
    async fn static_provider_answers_only_what_was_asked() {
        let p = fixed(&[(1, 1.5), (9, 9.5)]);
        assert_eq!(p.values(&[1]).await, Ok(HashMap::from([(1, 1.5)])));
    }

    #[tokio::test]
    async fn provider_values_win_and_db_fills_the_gaps() {
        let providers: Vec<Arc<dyn SignalProvider>> = vec![Arc::new(fixed(&[(1, 1.5)]))];
        let got = get_values(&providers, &[1, 2], &db(), TIMEOUT).await;
        assert_eq!(got, HashMap::from([(1, 1.5), (2, 20.0)]));
    }

    #[tokio::test]
    async fn failing_provider_falls_back_to_db() {
        let providers: Vec<Arc<dyn SignalProvider>> = vec![Arc::new(FailingProvider)];
        let got = get_values(&providers, &[1], &db(), TIMEOUT).await;
        assert_eq!(got, HashMap::from([(1, 10.0)]));
    }

    #[tokio::test(start_paused = true)]
    async fn hanging_provider_is_bounded_and_does_not_block_others() {
        let providers: Vec<Arc<dyn SignalProvider>> =
            vec![Arc::new(HangingProvider), Arc::new(fixed(&[(2, 2.5)]))];
        let start = tokio::time::Instant::now();
        let got = get_values(&providers, &[1, 2], &db(), TIMEOUT).await;
        assert_eq!(got, HashMap::from([(1, 10.0), (2, 2.5)]));
        assert_eq!(
            start.elapsed(),
            TIMEOUT,
            "waited exactly one timeout, not forever"
        );
    }

    #[tokio::test]
    async fn every_available_provider_is_called_once() {
        let counting = Arc::new(CountingProvider {
            calls: AtomicUsize::new(0),
            inner: fixed(&[(3, 3.5)]),
        });
        let providers: Vec<Arc<dyn SignalProvider>> =
            vec![counting.clone(), Arc::new(UnavailableProvider)];
        let got = get_values(&providers, &[3], &db(), TIMEOUT).await;
        assert_eq!(got, HashMap::from([(3, 3.5)]));
        assert_eq!(counting.calls.load(Ordering::SeqCst), 1);
    }
}
