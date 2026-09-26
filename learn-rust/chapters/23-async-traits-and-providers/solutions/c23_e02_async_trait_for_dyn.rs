// c23_e02_async_trait_for_dyn.rs
//
// CONCEPT: trait objects (`dyn Trait`) and async methods — why the
//          databroker uses `#[async_trait]`, and why its provider traits
//          require `Send + Sync + 'static`.
// GUIDE:   chapters/23-async-traits-and-providers/guide.md  (section "Why #[async_trait] for dyn")
// DATABROKER: databroker/src/broker.rs — `#[async_trait::async_trait] pub trait
//             SignalProvider: Send + Sync + 'static`, stored as
//             `Arc<dyn SignalProvider>` in `SignalProviderSubscription`
// EXPECT:  compile error E0038 (the trait `Provider` is not dyn compatible)
//
// The broker keeps providers of different concrete types in ONE collection,
// so it needs `dyn Provider`. A native `async fn` in a trait returns a
// different, unnameable future type per implementation — a vtable can't hold
// that, so the trait isn't "dyn compatible". `#[async_trait]` rewrites each
// `async fn` to return `Pin<Box<dyn Future<Output = ...> + Send + '_>>` —
// one nameable type — at the cost of one heap allocation per call.
//
// YOUR TASK
//   Make it compile and pass: annotate the trait AND both impls with
//   `#[async_trait::async_trait]`, and give the trait the supertraits the
//   test needs to move an `Arc<dyn Provider>` into `tokio::spawn`.
//   Do not edit the tests.

use std::sync::Arc;
use std::time::Duration;

// WHY `Send + Sync`: `Arc<dyn Provider>` is only `Send` if the object is
// `Send + Sync`, and tokio::spawn needs Send.
#[async_trait::async_trait]
trait Provider: Send + Sync {
    async fn value(&self, id: i32) -> f64;
    fn is_available(&self) -> bool;
}

struct Fixed(f64);

struct Delayed {
    value: f64,
    delay: Duration,
}

#[async_trait::async_trait]
impl Provider for Fixed {
    async fn value(&self, _id: i32) -> f64 {
        self.0
    }
    fn is_available(&self) -> bool {
        true
    }
}

#[async_trait::async_trait]
impl Provider for Delayed {
    async fn value(&self, _id: i32) -> f64 {
        tokio::time::sleep(self.delay).await;
        self.value
    }
    fn is_available(&self) -> bool {
        true
    }
}

fn providers() -> Vec<Arc<dyn Provider>> {
    vec![
        Arc::new(Fixed(1.0)),
        Arc::new(Delayed {
            value: 2.0,
            delay: Duration::from_millis(10),
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn mixed_providers_in_one_vec() {
        let mut total = 0.0;
        for provider in providers() {
            assert!(provider.is_available());
            total += provider.value(7).await;
        }
        assert_eq!(total, 3.0);
    }

    #[tokio::test(start_paused = true)]
    async fn providers_can_be_used_from_spawned_tasks() {
        let handles: Vec<_> = providers()
            .into_iter()
            .map(|provider| tokio::spawn(async move { provider.value(1).await }))
            .collect();
        let mut values = Vec::new();
        for handle in handles {
            values.push(handle.await.unwrap());
        }
        assert_eq!(values, vec![1.0, 2.0]);
    }
}
