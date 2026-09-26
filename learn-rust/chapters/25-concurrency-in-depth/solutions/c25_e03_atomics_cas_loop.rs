// c25_e03_atomics_cas_loop.rs
//
// CONCEPT: compare-and-swap (`compare_exchange`) loops — lock-free
//          read-modify-write of anything that fits in an atomic; `fetch_max`,
//          `fetch_update`; the ABA problem in words.
// GUIDE:   chapters/25-concurrency-in-depth/guide.md  (section "Compare-and-swap loops")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `next_request_id:
//             AtomicU32` with `fetch_add` (line 174) is the simplest atomic RMW.
//             crossbeam's bounded channel is built on `compare_exchange_weak`
//             loops (~/.cargo/registry/src/index.crates.io-*/crossbeam-channel-0.5.17/src/flavors/array.rs:194).
// EXPECT:  tests fail: `todo!()` panics
//
// `fetch_add` covers "add 1". For any other update — "store the max",
// "decrement but never below zero", "set only if unset" — you use a CAS loop:
//
//     let mut current = a.load(Relaxed);
//     loop {
//         let new = f(current);
//         match a.compare_exchange_weak(current, new, AcqRel, Relaxed) {
//             Ok(_) => break,                 // nobody interfered
//             Err(actual) => current = actual // someone did: retry with their value
//         }
//     }
//
// `_weak` may fail spuriously (cheaper on ARM); fine inside a loop.
//
// YOUR TASK
//   1. `record_max`: a CAS loop that raises `max` to `value` if larger.
//      Returns the previous max. (Don't use `fetch_max` — write the loop.)
//   2. `try_acquire`: a permit counter; take one permit if any are left,
//      never going below zero. Use `fetch_update`.
//   3. `set_once`: store `value` into `slot` only if `slot` is still 0; return
//      `Ok(())` if you won, `Err(existing)` if someone else set it first.
//   Do not edit the tests.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Raises `max` to `value` if larger; returns the value seen before.
fn record_max(max: &AtomicU64, value: u64) -> u64 {
    let mut current = max.load(Ordering::Relaxed);
    loop {
        if value <= current {
            return current; // nothing to do
        }
        // WHY: Relaxed is enough — the max is a standalone number that
        // doesn't publish any other data.
        match max.compare_exchange_weak(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(previous) => return previous,
            Err(actual) => current = actual,
        }
    }
}

/// Takes one permit if available. Returns whether it succeeded.
fn try_acquire(permits: &AtomicUsize) -> bool {
    // `fetch_update` is the CAS loop packaged up: the closure returns
    // `None` to give up, `Some(new)` to attempt the swap.
    permits
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
        .is_ok()
}

fn release(permits: &AtomicUsize) {
    permits.fetch_add(1, Ordering::AcqRel);
}

/// First writer wins. 0 means "unset".
fn set_once(slot: &AtomicU64, value: u64) -> Result<(), u64> {
    assert_ne!(value, 0, "0 is reserved for 'unset'");
    slot.compare_exchange(0, value, Ordering::AcqRel, Ordering::Acquire)
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn record_max_single_thread() {
        let m = AtomicU64::new(5);
        assert_eq!(record_max(&m, 3), 5);
        assert_eq!(m.load(Ordering::SeqCst), 5);
        assert_eq!(record_max(&m, 9), 5);
        assert_eq!(m.load(Ordering::SeqCst), 9);
    }

    #[test]
    fn record_max_under_contention() {
        let m = AtomicU64::new(0);
        let start = Barrier::new(8);
        thread::scope(|s| {
            for t in 0..8u64 {
                let (m, start) = (&m, &start);
                s.spawn(move || {
                    start.wait();
                    for i in 0..10_000u64 {
                        record_max(m, t * 10_000 + i);
                    }
                });
            }
        });
        assert_eq!(m.load(Ordering::SeqCst), 7 * 10_000 + 9_999);
    }

    #[test]
    fn permits_never_go_negative_and_are_all_accounted_for() {
        let permits = AtomicUsize::new(3);
        let acquired = AtomicUsize::new(0);
        let start = Barrier::new(16);
        thread::scope(|s| {
            for _ in 0..16 {
                s.spawn(|| {
                    start.wait();
                    if try_acquire(&permits) {
                        acquired.fetch_add(1, Ordering::SeqCst);
                    }
                });
            }
        });
        assert_eq!(acquired.load(Ordering::SeqCst), 3);
        assert_eq!(permits.load(Ordering::SeqCst), 0);
        release(&permits);
        assert!(try_acquire(&permits));
        assert!(!try_acquire(&permits));
    }

    #[test]
    fn set_once_exactly_one_winner() {
        let slot = AtomicU64::new(0);
        let start = Barrier::new(10);
        let winners: usize = thread::scope(|s| {
            let handles: Vec<_> = (1..=10u64)
                .map(|v| {
                    let (slot, start) = (&slot, &start);
                    s.spawn(move || {
                        start.wait();
                        set_once(slot, v).is_ok() as usize
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).sum()
        });
        assert_eq!(winners, 1);
        let value = slot.load(Ordering::SeqCst);
        assert!((1..=10).contains(&value));
        assert_eq!(set_once(&slot, 99), Err(value));
    }
}
