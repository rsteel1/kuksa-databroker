// c25_e05_deadlock_and_lock_ordering.rs
//
// CONCEPT: deadlocks from inconsistent lock order, preventing them with a
//          global lock order, and detecting/avoiding them with `try_lock`
//          plus back-off.
// GUIDE:   chapters/25-concurrency-in-depth/guide.md  (section "Deadlocks and lock ordering")
// DATABROKER: databroker/src/broker.rs — `update_entries` (line 1529) takes the
//             database lock, then the subscriptions lock; every other path that
//             needs both (e.g. `subscribe`, line 1601) must take them in the
//             same order or releases one before taking the other.
// EXPECT:  tests fail: `todo!()` panics
//
// Thread 1 locks A then B; thread 2 locks B then A. Each holds one lock and
// waits forever for the other. The fix is a rule every piece of code follows:
// "always lock in ascending id order". With that rule a cycle is impossible.
//
// When a global order is impossible (you don't know the second lock until you
// hold the first), use `try_lock`: if the second lock is busy, release the
// first, back off, and retry.
//
// The tests run the transfers on worker threads and wait with a timeout, so a
// deadlocking implementation FAILS the test instead of hanging.
//
// YOUR TASK
//   1. `transfer`: move `amount` between two accounts, locking both balances
//      in ascending `id` order (whatever the argument order). Transferring to
//      the same account is a no-op (don't lock the same Mutex twice!).
//      Insufficient funds -> `Err(InsufficientFunds)` and nothing changes.
//   2. `try_transfer`: same result, but lock `from` first, then `try_lock`
//      `to`; if that fails, drop everything, `thread::yield_now()`, and retry,
//      at most `max_attempts` times -> `Err(Busy)` when exhausted.
//   Do not edit the tests.

// I AM NOT DONE

use std::sync::{Mutex, TryLockError};
use std::thread;

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
    Busy,
}

fn transfer(from: &Account, to: &Account, amount: i64) -> Result<(), TransferError> {
    todo!()
}

fn try_transfer(
    from: &Account,
    to: &Account,
    amount: i64,
    max_attempts: usize,
) -> Result<(), TransferError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::time::Duration;

    /// Runs `f` on a thread; fails the test (instead of hanging) if it takes
    /// longer than 5 s — the symptom of a deadlock.
    fn within_timeout(f: impl FnOnce() + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            f();
            let _ = tx.send(());
        });
        rx.recv_timeout(Duration::from_secs(5))
            .expect("deadlock: transfers did not finish in time");
    }

    #[test]
    fn simple_transfers() {
        let a = Account::new(1, 100);
        let b = Account::new(2, 0);
        assert_eq!(transfer(&a, &b, 30), Ok(()));
        assert_eq!(transfer(&b, &a, 10), Ok(()));
        assert_eq!((a.balance(), b.balance()), (80, 20));
        assert_eq!(transfer(&b, &a, 21), Err(TransferError::InsufficientFunds));
        assert_eq!((a.balance(), b.balance()), (80, 20));
        assert_eq!(
            transfer(&a, &a, 5),
            Ok(()),
            "self transfer must not self-deadlock"
        );
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

    #[test]
    fn try_transfer_gives_up_when_the_target_stays_locked() {
        let a = Account::new(1, 50);
        let b = Account::new(2, 0);
        let held = b.balance.lock().unwrap();
        assert_eq!(try_transfer(&a, &b, 10, 3), Err(TransferError::Busy));
        drop(held);
        assert_eq!(a.balance(), 50, "nothing moved while busy");
        assert_eq!(try_transfer(&a, &b, 10, 3), Ok(()));
        assert_eq!((a.balance(), b.balance()), (40, 10));
    }

    #[test]
    fn try_transfer_in_opposite_directions_never_hangs() {
        let a = Arc::new(Account::new(1, 1_000_000));
        let b = Arc::new(Account::new(2, 1_000_000));
        within_timeout({
            let (a, b) = (a.clone(), b.clone());
            move || {
                thread::scope(|s| {
                    s.spawn(|| {
                        for _ in 0..5_000 {
                            let _ = try_transfer(&a, &b, 1, usize::MAX);
                        }
                    });
                    s.spawn(|| {
                        for _ in 0..5_000 {
                            let _ = try_transfer(&b, &a, 1, usize::MAX);
                        }
                    });
                });
            }
        });
        assert_eq!(a.balance() + b.balance(), 2_000_000);
    }
}
