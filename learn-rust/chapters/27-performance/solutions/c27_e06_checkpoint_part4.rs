// c27_e06_checkpoint_part4.rs
//
// CONCEPT: CHECKPOINT (Part IV). Design a read-mostly signal cache: cheap
//          consistent snapshots, allocation budgets, no unsafe in the API.
// GUIDE:   chapters/27-performance/guide.md  (section "Checkpoint")
// DATABROKER: databroker/src/broker.rs:179-186 — `DataBroker` guards its
//             `Database` with `Arc<RwLock<..>>`, so every reader takes the
//             lock. Here, readers take a lock only long enough to clone an
//             `Arc`, then read an immutable snapshot with no lock at all
//             (copy-on-write, the idea behind the `arc-swap` crate).
// EXPECT:  tests fail: todo!() panics
//
// Only signatures and tests are given. Choose the fields yourself. Draws on:
// Arc / RwLock / atomics (12, 25), `Arc<str>` and `Borrow` (14, 15),
// `with_capacity` and allocation counting (this chapter).
//
// REQUIREMENTS (the tests check all of them)
//   * `snapshot()` returns an immutable view that never changes, even if
//     `publish` runs afterwards. Taking one allocates nothing.
//   * `Snapshot::get(&str)` allocates nothing.
//   * `version()` doesn't lock: it increases by one per `publish`.
//   * `publish` of values for paths that are ALREADY known costs at most two
//     allocations, whatever the number of signals (hint: what does cloning a
//     `HashMap<Arc<str>, f64>` allocate?).
//   * Many threads may read while one publishes. No `unsafe` anywhere.
//
// YOUR TASK
//   Add fields to `SignalCache` and `Snapshot`, implement every `todo!()`.
//   Do not edit the tests.

#![allow(unsafe_code)] // only for the provided counting allocator below

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

// ---------------------------------------------------------------------------
// Provided: the counting allocator you built in c27_e01. It counts every
// allocation and reallocation made *by the current thread*, so tests running
// in parallel on other threads don't disturb the numbers.
// ---------------------------------------------------------------------------
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        // `const` initialisation: accessing it never allocates, so the
        // allocator can use it without recursing into itself.
        static ALLOCS: Cell<usize> = const { Cell::new(0) };
    }

    fn bump() {
        // `try_with` instead of `with`: never panic inside the allocator,
        // even while a thread is being torn down.
        let _ = ALLOCS.try_with(|count| count.set(count.get() + 1));
    }

    pub struct CountingAlloc;

    // SAFETY: every method forwards to `System` with the caller's arguments
    // unchanged, so all of `GlobalAlloc`'s contracts are upheld by `System`.
    // The only extra work touches a const-initialised thread-local `Cell`,
    // which neither allocates nor unwinds.
    unsafe impl GlobalAlloc for CountingAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `alloc`'s contract; passed through.
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `alloc_zeroed`'s contract; passed through.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `realloc`'s contract; passed through.
            unsafe { System.realloc(ptr, layout, new_size) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: `ptr` was allocated by `System` via the methods above
            // with this `layout`; the caller upholds `dealloc`'s contract.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static GLOBAL: CountingAlloc = CountingAlloc;

    /// Runs `f` and returns its result plus the number of (re)allocations
    /// it performed on the current thread.
    pub fn count_allocs<R>(f: impl FnOnce() -> R) -> (R, usize) {
        let before = ALLOCS.with(Cell::get);
        let result = f();
        let after = ALLOCS.with(Cell::get);
        (result, after - before)
    }
}
use counting::count_allocs;

#[derive(Clone)]
pub struct Snapshot {
    // WHY: one Arc'd immutable map; cloning a Snapshot is a refcount bump.
    values: Arc<HashMap<Arc<str>, f64>>,
}

impl Snapshot {
    pub fn get(&self, path: &str) -> Option<f64> {
        // WHY: `Arc<str>: Borrow<str>`, so a `&str` lookup needs no allocation.
        self.values.get(path).copied()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Default)]
pub struct SignalCache {
    current: RwLock<Arc<HashMap<Arc<str>, f64>>>,
    version: AtomicU64,
}

impl SignalCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Snapshot {
        // WHY: the lock is held only for an Arc clone, never across a read.
        let values = self
            .current
            .read()
            .expect("no writer panics while holding the lock")
            .clone();
        Snapshot { values }
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// Applies all `updates` atomically: readers see all of them or none.
    pub fn publish(&self, updates: &[(&str, f64)]) {
        let mut guard = self
            .current
            .write()
            .expect("no writer panics while holding the lock");
        // WHY: copy-on-write. Old snapshots keep the old Arc; cloning the map
        // clones Arc<str> keys (refcount bumps), so it is one table allocation.
        let mut next: HashMap<Arc<str>, f64> = (**guard).clone();
        for (path, value) in updates {
            match next.get_mut(*path) {
                Some(slot) => *slot = *value,
                None => {
                    next.insert(Arc::from(*path), *value);
                }
            }
        }
        *guard = Arc::new(next);
        // WHY: Release pairs with the Acquire in `version()`, so a reader that
        // sees the new version also sees the new map on its next snapshot.
        self.version.fetch_add(1, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> SignalCache {
        let cache = SignalCache::new();
        let paths: Vec<String> = (0..200).map(|i| format!("Vehicle.Signal{i}")).collect();
        let updates: Vec<(&str, f64)> = paths.iter().map(|p| (p.as_str(), 0.0)).collect();
        cache.publish(&updates);
        cache
    }

    #[test]
    fn publish_then_read() {
        let cache = SignalCache::new();
        assert!(cache.snapshot().is_empty());
        cache.publish(&[("Vehicle.Speed", 88.5), ("Vehicle.Cabin.Temp", 21.0)]);
        let snap = cache.snapshot();
        assert_eq!(snap.get("Vehicle.Speed"), Some(88.5));
        assert_eq!(snap.get("Vehicle.Nope"), None);
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn snapshots_are_immutable() {
        let cache = SignalCache::new();
        cache.publish(&[("Vehicle.Speed", 1.0)]);
        let old = cache.snapshot();
        cache.publish(&[("Vehicle.Speed", 2.0), ("Vehicle.New", 3.0)]);
        assert_eq!(old.get("Vehicle.Speed"), Some(1.0));
        assert_eq!(old.get("Vehicle.New"), None);
        assert_eq!(cache.snapshot().get("Vehicle.Speed"), Some(2.0));
    }

    #[test]
    fn version_counts_publishes() {
        let cache = SignalCache::new();
        assert_eq!(cache.version(), 0);
        cache.publish(&[("A", 1.0)]);
        cache.publish(&[]);
        assert_eq!(cache.version(), 2);
    }

    #[test]
    fn reads_do_not_allocate() {
        let cache = seeded();
        let (hits, n) = count_allocs(|| {
            let snap = cache.snapshot();
            (0..200)
                .filter(|_| snap.get("Vehicle.Signal7").is_some())
                .count()
        });
        assert_eq!(hits, 200);
        assert_eq!(n, 0, "snapshot + get allocated {n} times");
    }

    #[test]
    fn republishing_known_paths_is_cheap() {
        let cache = seeded();
        let paths: Vec<String> = (0..200).map(|i| format!("Vehicle.Signal{i}")).collect();
        let updates: Vec<(&str, f64)> = paths.iter().map(|p| (p.as_str(), 1.0)).collect();
        let (_, n) = count_allocs(|| cache.publish(&updates));
        assert!(
            n <= 2,
            "publish of 200 known paths allocated {n} times (budget 2)"
        );
        assert_eq!(cache.snapshot().get("Vehicle.Signal199"), Some(1.0));
    }

    #[test]
    fn readers_and_a_writer_in_parallel() {
        let cache = seeded();
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| {
                    for _ in 0..200 {
                        let snap = cache.snapshot();
                        // All-or-nothing: both signals always carry the same round.
                        let a = snap.get("Vehicle.Signal1").expect("seeded");
                        let b = snap.get("Vehicle.Signal2").expect("seeded");
                        assert_eq!(a, b, "saw a half-applied publish");
                    }
                });
            }
            s.spawn(|| {
                for round in 1..=100 {
                    let v = f64::from(round);
                    cache.publish(&[("Vehicle.Signal1", v), ("Vehicle.Signal2", v)]);
                }
            });
        });
        assert_eq!(cache.version(), 101);
    }
}
