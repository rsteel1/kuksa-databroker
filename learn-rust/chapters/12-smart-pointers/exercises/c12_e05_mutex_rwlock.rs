// c12_e05_mutex_rwlock.rs
//
// CONCEPT: `Mutex<T>` / `RwLock<T>` — interior mutability that is safe across
//          threads; guards; keeping critical sections short; snapshots.
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Mutex and RwLock: guards release locks")
// DATABROKER: databroker/src/broker.rs — `database: Arc<RwLock<Database>>`
//          (line 180). The databroker uses *tokio's* RwLock (async; chapter
//          14), but guards behave the same: the lock is held exactly as long
//          as the guard value lives. `snapshot_signal_providers` (line 2445)
//          copies what it needs out of the lock and releases it before
//          doing slow work.
// EXPECT:  tests fail: `todo!()` panics.
//
// `lock()` / `read()` / `write()` return a GUARD. Deref the guard to reach the
// data; drop the guard to unlock. A guard kept in a variable holds the lock
// until the end of the scope — a classic source of contention and deadlock.
//
// std's locks return `Result` because a thread that panicked while holding
// the lock "poisons" it. Here, `.expect("lock poisoned")` is appropriate: a
// poisoned lock means a bug already happened.
//
// YOUR TASK
//   Implement the `todo!()`s. Every method takes `&self` — the lock provides
//   the mutability. The tests check that no guard is left held afterwards.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

#[derive(Default)]
struct SharedDb {
    values: RwLock<HashMap<String, i64>>,
    writes: Mutex<u64>,
}

impl SharedDb {
    /// Inserts/overwrites and bumps the `writes` counter.
    fn set(&self, path: &str, value: i64) {
        todo!()
    }

    /// Reads one value (copied out, so no guard escapes).
    fn value(&self, path: &str) -> Option<i64> {
        todo!()
    }

    /// Copies the whole map out so slow work can happen without the lock.
    fn snapshot(&self) -> HashMap<String, i64> {
        todo!()
    }

    /// Runs `f` while holding the read lock and returns its result.
    fn with_read<T>(&self, f: impl FnOnce(&HashMap<String, i64>) -> T) -> T {
        todo!()
    }

    fn writes(&self) -> u64 {
        *self.writes.lock().expect("lock poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_unlocked(db: &SharedDb) {
        assert!(
            db.values.try_write().is_ok(),
            "a guard on `values` is still held"
        );
        assert!(
            db.writes.try_lock().is_ok(),
            "a guard on `writes` is still held"
        );
    }

    #[test]
    fn set_and_read() {
        let db = SharedDb::default();
        db.set("Vehicle.Speed", 50);
        db.set("Vehicle.Speed", 60);
        db.set("Vehicle.Horn", 1);
        assert_eq!(db.value("Vehicle.Speed"), Some(60));
        assert_eq!(db.value("Nope"), None);
        assert_eq!(db.writes(), 3);
        assert_unlocked(&db);
    }

    #[test]
    fn snapshot_is_independent() {
        let db = SharedDb::default();
        db.set("A", 1);
        let snap = db.snapshot();
        db.set("A", 2);
        assert_eq!(snap["A"], 1);
        assert_unlocked(&db);
    }

    #[test]
    fn with_read_releases_the_lock() {
        let db = SharedDb::default();
        db.set("A", 1);
        db.set("B", 2);
        let total: i64 = db.with_read(|map| map.values().sum());
        assert_eq!(total, 3);
        assert_unlocked(&db);
    }

    #[test]
    fn works_from_many_threads() {
        let db = SharedDb::default();
        std::thread::scope(|s| {
            for t in 0..4 {
                let db = &db;
                s.spawn(move || {
                    for i in 0..100 {
                        db.set(&format!("T{t}.S{i}"), i);
                    }
                });
            }
        });
        assert_eq!(db.writes(), 400);
        assert_eq!(db.snapshot().len(), 400);
    }
}
