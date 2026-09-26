// c21_e06_snapshot_then_await.rs
//
// CONCEPT: never hold a lock guard while awaiting something you don't
//          control. Snapshot what you need (cheap `Arc` clones), drop the
//          guard, THEN await.
// GUIDE:   chapters/21-async-locks-and-channels/guide.md  (section "Snapshot, then await")
// DATABROKER: databroker/src/broker.rs — `snapshot_actuation_subscriptions`,
//             `snapshot_signal_providers` and the `let providers = { ... }`
//             block at the top of `get_values_broker` (commit e1fdfd1 "Make sure
//             blocking/slow providers can not globally block other databroker
//             operations")
// EXPECT:  compiles, but the test `registering_is_not_blocked_by_a_hung_provider`
//          fails (the register call times out)
//
// Before commit e1fdfd1, `actuate` held `subscriptions.read()` while awaiting
// `provider.actuate(...)`. A provider that never answered kept that read lock
// forever, so anything needing `subscriptions.write()` (new subscribers,
// cleanup) hung — one bad client could freeze the whole broker.
//
// YOUR TASK
//   Rewrite `Registry::query_all` so the read guard is released before any
//   provider is awaited. Keep the answers in registration order.
//   Do not edit the tests.

// I AM NOT DONE

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

struct SlowProvider {
    answer: i32,
    delay: Duration,
}

impl SlowProvider {
    async fn query(&self) -> i32 {
        tokio::time::sleep(self.delay).await;
        self.answer
    }
}

#[derive(Default)]
struct Registry {
    providers: RwLock<Vec<Arc<SlowProvider>>>,
}

impl Registry {
    async fn register(&self, provider: SlowProvider) {
        self.providers.write().await.push(Arc::new(provider));
    }

    async fn query_all(&self) -> Vec<i32> {
        let providers = self.providers.read().await;
        let mut answers = Vec::new();
        for provider in providers.iter() {
            answers.push(provider.query().await); // read guard held across await!
        }
        answers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn registering_is_not_blocked_by_a_hung_provider() {
        let registry = Arc::new(Registry::default());
        registry
            .register(SlowProvider {
                answer: 1,
                delay: Duration::from_secs(3600),
            })
            .await;

        let query = {
            let registry = registry.clone();
            tokio::spawn(async move { registry.query_all().await })
        };
        tokio::time::sleep(Duration::from_millis(10)).await; // query is now waiting

        let register = registry.register(SlowProvider {
            answer: 2,
            delay: Duration::ZERO,
        });
        tokio::time::timeout(Duration::from_millis(100), register)
            .await
            .expect("register() was blocked by a lock held across .await");

        query.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn answers_in_registration_order() {
        let registry = Registry::default();
        registry
            .register(SlowProvider {
                answer: 1,
                delay: Duration::from_millis(20),
            })
            .await;
        registry
            .register(SlowProvider {
                answer: 2,
                delay: Duration::from_millis(10),
            })
            .await;
        assert_eq!(registry.query_all().await, vec![1, 2]);
    }
}
