// c23_e03_provider_registry.rs
//
// CONCEPT: a registry of `Arc<dyn Trait>` objects keyed by `Uuid`, with
//          liveness checks and `retain`-based cleanup.
// GUIDE:   chapters/23-async-traits-and-providers/guide.md  (section "The provider registry")
// DATABROKER: databroker/src/broker.rs — `Subscriptions::add_signal_provider_subscription`
//             (returns a `Uuid`), `cleanup_signal_providers_subscriptions`
//             (`retain` + `is_available()`), and `register_signals`
//             (a signal may only have one provider)
// EXPECT:  tests fail: `todo!()` panics
//
// A provider is "available" as long as its gRPC stream is open. In the real
// broker, `Provider::is_available` is `!self.sender.is_closed()`; here the
// test provider just has an `AtomicBool` the test can flip.
//
// YOUR TASK
//   Implement `Registry::register`, `provider_for` and `cleanup`.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait::async_trait]
trait SignalProvider: Send + Sync {
    fn is_available(&self) -> bool;
    async fn value(&self, id: i32) -> Option<f64>;
}

#[derive(Debug, PartialEq)]
enum RegisterError {
    /// Some other provider already provides this signal.
    AlreadyProvided(i32),
}

struct Registration {
    provider: Arc<dyn SignalProvider>,
    signals: HashSet<i32>,
}

#[derive(Default)]
struct Registry {
    providers: HashMap<Uuid, Registration>,
}

impl Registry {
    /// Register `provider` for `signals`. Fails (and registers nothing) if
    /// any of the signals is already provided by someone else.
    fn register(
        &mut self,
        provider: Arc<dyn SignalProvider>,
        signals: HashSet<i32>,
    ) -> Result<Uuid, RegisterError> {
        todo!()
    }

    /// The provider responsible for `signal`, if any. Cloning the `Arc` lets
    /// the caller use it after releasing any lock around the registry.
    fn provider_for(&self, signal: i32) -> Option<Arc<dyn SignalProvider>> {
        todo!()
    }

    /// Remove every provider that is no longer available. Returns the ids of
    /// the removed providers and the signals they used to provide.
    fn cleanup(&mut self) -> HashMap<Uuid, HashSet<i32>> {
        todo!()
    }
}

struct TestProvider {
    available: AtomicBool,
    value: f64,
}

impl TestProvider {
    fn new(value: f64) -> Arc<Self> {
        Arc::new(TestProvider {
            available: AtomicBool::new(true),
            value,
        })
    }
}

#[async_trait::async_trait]
impl SignalProvider for TestProvider {
    fn is_available(&self) -> bool {
        self.available.load(Ordering::SeqCst)
    }
    async fn value(&self, _id: i32) -> Option<f64> {
        Some(self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_and_lookup() {
        let mut registry = Registry::default();
        registry
            .register(TestProvider::new(1.0), HashSet::from([1, 2]))
            .unwrap();
        registry
            .register(TestProvider::new(3.0), HashSet::from([3]))
            .unwrap();

        let provider = registry.provider_for(3).expect("signal 3 is provided");
        assert_eq!(provider.value(3).await, Some(3.0));
        assert!(registry.provider_for(4).is_none());
    }

    #[test]
    fn signals_have_at_most_one_provider() {
        let mut registry = Registry::default();
        registry
            .register(TestProvider::new(1.0), HashSet::from([1, 2]))
            .unwrap();
        let result = registry.register(TestProvider::new(2.0), HashSet::from([5, 2]));
        assert_eq!(result, Err(RegisterError::AlreadyProvided(2)));
        // Nothing from the failed registration was stored.
        assert!(registry.provider_for(5).is_none());
    }

    #[test]
    fn cleanup_removes_disconnected_providers() {
        let mut registry = Registry::default();
        let alive = TestProvider::new(1.0);
        let gone = TestProvider::new(2.0);
        registry.register(alive, HashSet::from([1])).unwrap();
        let gone_id = registry
            .register(gone.clone(), HashSet::from([2, 3]))
            .unwrap();

        assert!(registry.cleanup().is_empty());

        gone.available.store(false, Ordering::SeqCst); // "stream closed"
        let removed = registry.cleanup();
        assert_eq!(removed, HashMap::from([(gone_id, HashSet::from([2, 3]))]));
        assert!(registry.provider_for(2).is_none());
        assert!(registry.provider_for(1).is_some());
        // Signals are free again.
        assert!(registry
            .register(TestProvider::new(9.0), HashSet::from([2]))
            .is_ok());
    }
}
