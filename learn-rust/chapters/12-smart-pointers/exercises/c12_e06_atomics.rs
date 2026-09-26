// c12_e06_atomics.rs
//
// CONCEPT: atomic integers/booleans — lock-free shared counters and flags;
//          `fetch_add`; memory `Ordering` (Relaxed vs Acquire/Release).
// GUIDE:   chapters/12-smart-pointers/guide.md  (section "Atomics and memory ordering")
// DATABROKER: databroker/src/broker.rs — `Database { next_id: AtomicI32, ... }`
//          (line 129) and `self.db.next_id.fetch_add(1, Ordering::SeqCst)`
//          (line 1326); databroker/src/grpc/kuksa_val_v2/val.rs —
//          `next_request_id: AtomicU32` with `fetch_add(1, Ordering::Relaxed)`
//          (line 174).
// EXPECT:  compile error E0277: `Cell<i32>` cannot be shared between threads
//          safely (Cell is not `Sync`).
//
// `Cell` gives interior mutability but only on ONE thread. For a number or
// flag shared by threads you don't need a Mutex: atomics do a single
// read-modify-write in one indivisible CPU instruction.
//
// Ordering, in one paragraph: `Relaxed` guarantees the operation itself is
// atomic (no lost increments, no duplicate ids) but promises nothing about
// OTHER memory. That's enough for an id counter. When a flag *publishes*
// other data ("config is ready, go read it"), the writer uses `Release` and
// the reader `Acquire` so the other data is guaranteed visible too.
// `SeqCst` is the strongest (and a safe default when unsure).
//
// YOUR TASK
//   1. Replace the `Cell`s with atomics (`AtomicI32`, `AtomicBool`) so the
//      types are `Sync`.
//   2. Implement the methods with the weakest correct ordering and explain
//      your choice to yourself.
//   Do not edit the tests.

// I AM NOT DONE

use std::cell::Cell;
use std::collections::HashSet;

struct IdAllocator {
    next: Cell<i32>,
}

impl IdAllocator {
    fn new(first: i32) -> Self {
        IdAllocator {
            next: Cell::new(first),
        }
    }

    /// Returns a fresh id; never returns the same id twice, even when called
    /// concurrently.
    fn allocate(&self) -> i32 {
        let id = self.next.get();
        self.next.set(id + 1);
        id
    }
}

struct ShutdownFlag {
    requested: Cell<bool>,
}

impl ShutdownFlag {
    fn new() -> Self {
        ShutdownFlag {
            requested: Cell::new(false),
        }
    }
    fn request(&self) {
        self.requested.set(true);
    }
    fn is_requested(&self) -> bool {
        self.requested.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_across_threads() {
        let ids = IdAllocator::new(1);
        let all: Vec<i32> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..4)
                .map(|_| s.spawn(|| (0..1000).map(|_| ids.allocate()).collect::<Vec<_>>()))
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("thread panicked"))
                .collect()
        });
        let unique: HashSet<i32> = all.iter().copied().collect();
        assert_eq!(all.len(), 4000);
        assert_eq!(unique.len(), 4000);
        assert_eq!(ids.allocate(), 4001);
    }

    #[test]
    fn workers_stop_on_shutdown() {
        let flag = ShutdownFlag::new();
        std::thread::scope(|s| {
            let worker = s.spawn(|| {
                let mut spins = 0u64;
                while !flag.is_requested() {
                    spins += 1;
                    std::hint::spin_loop();
                }
                spins
            });
            flag.request();
            worker.join().expect("worker panicked");
        });
        assert!(flag.is_requested());
    }
}
