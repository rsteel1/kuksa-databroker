// c25_e08_review_ordering_and_lock_order.rs
//
// CONCEPT: code review — this file COMPILES, but has 2 planted bugs typical
//          of chapter 25 (a "ready" flag published with `Ordering::Relaxed`
//          where `Release`/`Acquire` is required, and a transfer that locks
//          two accounts in whatever order its arguments arrived instead of a
//          consistent global order, which can deadlock).
// GUIDE:   chapters/25-concurrency-in-depth/guide.md  (sections "Memory
//          ordering in practice" and "Deadlocks and lock ordering")
// DATABROKER: databroker/src/broker.rs — `next_id.fetch_add(1, Ordering::SeqCst)`
//             (line 1326) is a deliberate ordering choice; `update_entries`
//             (line 1529) always takes the database lock before the
//             subscriptions lock, and every other path that needs both must
//             agree on that order or it can deadlock against `update_entries`.
// EXPECT:  tests fail: a reasoning check on which orderings must guard the
//          "ready" flag, and a transfer test that hangs (deadlocks) under
//          this file's timeout harness.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// --------------------------------------------------------------- Snapshot

/// One writer publishes a two-counter snapshot; a reader only trusts the
/// counters once it observes `ready`. `calls`/`errors` can stay `Relaxed`:
/// once the *flag* synchronizes, everything written before it in program
/// order (atomic or not) becomes visible too. The flag itself is what needs
/// to actually establish that happens-before edge.
struct Snapshot {
    calls: AtomicU64,
    errors: AtomicU64,
    ready: AtomicBool,
}

/// Which orderings must guard the `ready` flag's store (the publishing
/// side) and load (the reading side) so that a reader who observes
/// `ready == true` also sees the counters written before it.
fn ready_flag_orderings() -> (Ordering, Ordering) {
    (Ordering::Relaxed, Ordering::Relaxed)
}

impl Snapshot {
    fn new() -> Self {
        Snapshot {
            calls: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            ready: AtomicBool::new(false),
        }
    }

    fn publish(&self, calls: u64, errors: u64) {
        let (store_ordering, _) = ready_flag_orderings();
        self.calls.store(calls, Ordering::Relaxed);
        self.errors.store(errors, Ordering::Relaxed);
        self.ready.store(true, store_ordering);
    }

    fn try_read(&self) -> Option<(u64, u64)> {
        let (_, load_ordering) = ready_flag_orderings();
        if self.ready.load(load_ordering) {
            Some((
                self.calls.load(Ordering::Relaxed),
                self.errors.load(Ordering::Relaxed),
            ))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------- Account

struct Account {
    id: u32,
    balance: Mutex<i64>,
}

impl Account {
    fn new(id: u32, balance: i64) -> Self {
        Account {
            id,
            balance: Mutex::new(balance),
        }
    }

    fn balance(&self) -> i64 {
        *self.balance.lock().expect("poisoned")
    }
}

#[derive(Debug, PartialEq)]
enum TransferError {
    InsufficientFunds,
}

/// Moves `amount` from `from` to `to`.
fn transfer(from: &Account, to: &Account, amount: i64) -> Result<(), TransferError> {
    let mut from_bal = from.balance.lock().expect("poisoned");
    let mut to_bal = to.balance.lock().expect("poisoned");
    if *from_bal < amount {
        return Err(TransferError::InsufficientFunds);
    }
    *from_bal -= amount;
    *to_bal += amount;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_flag_uses_release_acquire() {
        assert_eq!(
            ready_flag_orderings(),
            (Ordering::Release, Ordering::Acquire),
            "publishing through a flag needs Release on the store and \
             Acquire on the load, or the reader has no guarantee it sees \
             the writes made before the flag was set"
        );
    }

    #[test]
    fn snapshot_basic_contract() {
        let s = Snapshot::new();
        assert_eq!(s.try_read(), None);
        s.publish(42, 1);
        assert_eq!(s.try_read(), Some((42, 1)));
    }

    #[test]
    fn simple_transfer() {
        let a = Account::new(1, 100);
        let b = Account::new(2, 0);
        assert_eq!(transfer(&a, &b, 30), Ok(()));
        assert_eq!((a.balance(), b.balance()), (70, 30));
        assert_eq!(transfer(&b, &a, 100), Err(TransferError::InsufficientFunds));
    }

    /// Runs `f` on a thread; fails the test (instead of hanging forever) if
    /// it takes longer than 5 s — the symptom of a deadlock.
    fn within_timeout(f: impl FnOnce() + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            f();
            let _ = tx.send(());
        });
        rx.recv_timeout(Duration::from_secs(5))
            .expect("deadlock: opposite-direction transfers did not finish in time");
    }

    #[test]
    fn opposite_direction_transfers_do_not_deadlock() {
        let a = Arc::new(Account::new(1, 1_000_000));
        let b = Arc::new(Account::new(2, 1_000_000));
        within_timeout({
            let (a, b) = (a.clone(), b.clone());
            move || {
                thread::scope(|s| {
                    s.spawn(|| {
                        for _ in 0..20_000 {
                            transfer(&a, &b, 1).unwrap();
                        }
                    });
                    s.spawn(|| {
                        for _ in 0..20_000 {
                            transfer(&b, &a, 1).unwrap();
                        }
                    });
                });
            }
        });
        assert_eq!(a.balance() + b.balance(), 2_000_000, "money is conserved");
    }
}
