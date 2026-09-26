// c21_e05_rwlock_downgrade.rs
//
// CONCEPT: `tokio::sync::RwLock` — many readers OR one writer — and
//          `RwLockWriteGuard::downgrade` to keep a consistent view while
//          letting readers back in.
// GUIDE:   chapters/21-async-locks-and-channels/guide.md  (section "tokio RwLock and downgrade")
// DATABROKER: databroker/src/broker.rs — `update_entries`: take the database
//             write lock, apply updates, then `let db = db.downgrade();` and
//             notify subscribers while holding only a read lock
// EXPECT:  tests fail: `todo!()` panics
//
// Why downgrade instead of "drop the write guard, then take a read guard"?
// Between those two steps another writer could sneak in, and subscribers
// would be notified about a state that already changed. `downgrade()` turns
// the write guard into a read guard atomically: readers may now enter,
// writers still can't.
//
// YOUR TASK
//   Implement `Store::set_and_notify`: insert `value` at `path` under the
//   write lock, downgrade, then "notify" (simulated by
//   `tokio::time::sleep(notify_time)`) while holding the read guard, and
//   finally return the number of entries seen by the notifier.
//   Do not edit the tests.

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{RwLock, RwLockWriteGuard};

#[derive(Default)]
struct Store {
    data: RwLock<HashMap<String, i32>>,
}

impl Store {
    async fn set_and_notify(&self, path: &str, value: i32, notify_time: Duration) -> usize {
        let mut write_guard = self.data.write().await;
        write_guard.insert(path.to_string(), value);
        let read_guard = RwLockWriteGuard::downgrade(write_guard);
        tokio::time::sleep(notify_time).await; // "notify subscribers"
        read_guard.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test(start_paused = true)]
    async fn readers_allowed_writers_blocked_during_notify() {
        let store = Arc::new(Store::default());
        let task = {
            let store = store.clone();
            tokio::spawn(async move {
                store
                    .set_and_notify("Vehicle.Speed", 50, Duration::from_millis(100))
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(10)).await; // now inside "notify"

        // Readers can get in (after downgrade)...
        assert_eq!(
            store
                .data
                .try_read()
                .expect("readers allowed")
                .get("Vehicle.Speed"),
            Some(&50)
        );
        // ...but writers cannot: the notifier still sees a consistent state.
        assert!(store.data.try_write().is_err());

        assert_eq!(task.await.unwrap(), 1);
        assert!(store.data.try_write().is_ok());
    }

    #[tokio::test]
    async fn counts_entries() {
        let store = Store::default();
        store.set_and_notify("a", 1, Duration::ZERO).await;
        assert_eq!(store.set_and_notify("b", 2, Duration::ZERO).await, 2);
    }
}
