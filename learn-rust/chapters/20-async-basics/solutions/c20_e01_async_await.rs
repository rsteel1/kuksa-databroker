// c20_e01_async_await.rs
//
// CONCEPT: `async fn` returns a *future*; nothing happens until you `.await` it.
// GUIDE:   chapters/20-async-basics/guide.md  (section "Futures are lazy state machines")
// DATABROKER: databroker/src/broker.rs — every `AuthorizedAccess` method is
//             `async fn` and every call site ends in `.await`
// EXPECT:  compile error E0369 (cannot add `impl Future<Output = u32>` to ...)
//
// Calling an `async fn` does NOT run it. It builds a value (a future) that
// describes the work. Only `.await` (or an executor such as Tokio) drives
// it to completion. In the databroker you'll see chains like
// `self.broker.database.read().await` — the `.await` is where the work happens.
//
// YOUR TASK
//   Fix `total_speed` so it waits for both readings and adds them.
//   Read the `futures_are_lazy` test: it passes already — make sure you
//   understand WHY before removing the marker.
//   Do not edit the tests.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Pretend to ask a sensor for a speed reading.
async fn fetch_speed(sensor: u32) -> u32 {
    sensor * 10
}

async fn total_speed() -> u32 {
    // WHY: `fetch_speed(1)` is a future; `.await` turns it into the u32.
    let front = fetch_speed(1).await;
    let rear = fetch_speed(2).await;
    front + rear
}

/// Returns a future that bumps `counter` when (and only when) it runs.
async fn bump(counter: Arc<AtomicUsize>) {
    counter.fetch_add(1, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn adds_both_readings() {
        assert_eq!(total_speed().await, 30);
    }

    #[tokio::test]
    async fn futures_are_lazy() {
        let counter = Arc::new(AtomicUsize::new(0));
        let future = bump(counter.clone());
        // Created, but not polled yet: nothing has happened.
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        future.await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        // A future that is dropped without being awaited never runs at all.
        drop(bump(counter.clone()));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
