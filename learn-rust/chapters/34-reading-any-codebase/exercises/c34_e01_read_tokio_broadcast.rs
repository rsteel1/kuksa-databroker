// c34_e01_read_tokio_broadcast.rs
//
// CONCEPT: read-then-rebuild. Read how `tokio::sync::broadcast` really
//          works, then rebuild its core (a ring buffer of positions, a
//          per-receiver cursor, and the "lagged" rule) in plain std Rust.
// GUIDE:   chapters/34-reading-any-codebase/guide.md  (section "Worked example: tokio::sync::broadcast")
// DATABROKER: databroker/src/broker.rs — `subscribe` builds every subscription on
//             `broadcast::channel(capacity)` and turns `Lagged(n)` into `None`.
// EXPECT:  tests fail: `todo!()` panics
//
// READ FIRST (tokio 1.53.1, under ~/.cargo/registry/src/index.crates.io-*/tokio-1.53.1/):
//   src/sync/broadcast.rs:370   `struct Shared` — `buffer`, `mask`, `tail`
//   src/sync/broadcast.rs:391   `struct Tail` — `pos`, `rx_cnt`, `closed`
//   src/sync/broadcast.rs:583   capacity is rounded up with `next_power_of_two`
//   src/sync/broadcast.rs:661   `Sender::send` — fails when `rx_cnt == 0`
//   src/sync/broadcast.rs:1257  `Receiver::recv_ref` — the lag rule at 1335-1355
// The real code adds per-slot locks, wakers and unsafe cells for async
// waiting. Ignore those: the *logic* is what you're rebuilding.
//
// YOUR TASK
//   Implement `channel`, `Sender::send` and `Receiver::try_recv` so the
//   mini channel behaves exactly like tokio's (the last test runs the same
//   script against both). Do not edit the tests.

// I AM NOT DONE

use std::sync::{Arc, Mutex};

#[derive(Debug, PartialEq, Eq)]
pub struct SendError<T>(pub T);

#[derive(Debug, PartialEq, Eq)]
pub enum TryRecvError {
    Empty,
    Closed,
    Lagged(u64),
}

struct Shared<T> {
    /// `buffer[pos & mask]` holds the value sent at position `pos`.
    buffer: Vec<Option<T>>,
    mask: usize,
    /// Position the next `send` will write to (total values ever sent).
    tail: u64,
    rx_cnt: usize,
    /// Set when the Sender is dropped.
    closed: bool,
}

pub struct Sender<T> {
    shared: Arc<Mutex<Shared<T>>>,
}

pub struct Receiver<T> {
    shared: Arc<Mutex<Shared<T>>>,
    /// Position of the next value this receiver will read.
    next: u64,
}

/// Creates a channel. `capacity` is rounded up to a power of two, like tokio.
///
/// # Panics
/// If `capacity` is zero (tokio panics too).
pub fn channel<T: Clone>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    todo!()
}

impl<T: Clone> Sender<T> {
    /// Stores `value` and returns how many receivers will see it.
    /// With no receivers the value is handed back in `SendError`.
    pub fn send(&self, value: T) -> Result<usize, SendError<T>> {
        todo!()
    }

    /// A new receiver sees only values sent *after* it subscribed.
    pub fn subscribe(&self) -> Receiver<T> {
        let mut shared = self.shared.lock().expect("lock not poisoned");
        shared.rx_cnt += 1;
        Receiver {
            shared: Arc::clone(&self.shared),
            next: shared.tail,
        }
    }

    pub fn receiver_count(&self) -> usize {
        self.shared.lock().expect("lock not poisoned").rx_cnt
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.closed = true;
        }
    }
}

impl<T: Clone> Receiver<T> {
    /// Tokio's rules:
    /// - nothing new: `Closed` if the sender is gone, else `Empty`;
    /// - fell behind by more than the capacity: jump to the oldest value
    ///   still stored and return `Lagged(number_skipped)`;
    /// - otherwise return a clone of the next value and advance.
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        todo!()
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.rx_cnt -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast as real;

    #[test]
    fn delivers_in_order() {
        let (tx, mut rx) = channel(4);
        assert_eq!(tx.send(1), Ok(1));
        assert_eq!(tx.send(2), Ok(1));
        assert_eq!(rx.try_recv(), Ok(1));
        assert_eq!(rx.try_recv(), Ok(2));
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn capacity_rounds_up_to_a_power_of_two() {
        let (tx, mut rx) = channel(3); // really 4
        for i in 0..4 {
            tx.send(i).expect("receiver alive");
        }
        assert_eq!(rx.try_recv(), Ok(0), "4 values fit, nothing lost");
    }

    #[test]
    fn slow_receivers_lag_and_resume_at_the_oldest_value() {
        let (tx, mut rx) = channel(2);
        for i in 1..=5 {
            tx.send(i).expect("receiver alive");
        }
        assert_eq!(rx.try_recv(), Err(TryRecvError::Lagged(3)));
        assert_eq!(rx.try_recv(), Ok(4));
        assert_eq!(rx.try_recv(), Ok(5));
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn subscribers_start_at_the_tail() {
        let (tx, mut rx1) = channel(4);
        tx.send("before").expect("receiver alive");
        let mut rx2 = tx.subscribe();
        assert_eq!(tx.receiver_count(), 2);
        tx.send("after").expect("receivers alive");
        assert_eq!(rx1.try_recv(), Ok("before"));
        assert_eq!(rx2.try_recv(), Ok("after"));
        assert_eq!(rx2.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn send_fails_without_receivers_and_closed_after_sender_drop() {
        let (tx, rx) = channel::<u8>(2);
        drop(rx);
        assert_eq!(tx.receiver_count(), 0);
        assert_eq!(tx.send(7), Err(SendError(7)));

        let (tx, mut rx) = channel(2);
        tx.send(1).expect("receiver alive");
        drop(tx);
        assert_eq!(rx.try_recv(), Ok(1), "buffered values survive the sender");
        assert_eq!(rx.try_recv(), Err(TryRecvError::Closed));
    }

    #[derive(Debug, PartialEq)]
    enum Seen {
        Value(u32),
        Empty,
        Closed,
        Lagged(u64),
    }

    #[test]
    fn behaves_exactly_like_tokio() {
        // Script: send `n` values, then read `r` times; repeat.
        let script: [(u32, usize); 4] = [(3, 1), (9, 3), (1, 5), (6, 2)];
        let capacity = 3;

        let (real_tx, mut real_rx) = real::channel(capacity);
        let (mini_tx, mut mini_rx) = channel(capacity);
        let mut real_seen = Vec::new();
        let mut mini_seen = Vec::new();
        let mut value = 0;
        for (sends, reads) in script {
            for _ in 0..sends {
                value += 1;
                real_tx.send(value).expect("receiver alive");
                mini_tx.send(value).expect("receiver alive");
            }
            for _ in 0..reads {
                real_seen.push(match real_rx.try_recv() {
                    Ok(v) => Seen::Value(v),
                    Err(real::error::TryRecvError::Empty) => Seen::Empty,
                    Err(real::error::TryRecvError::Closed) => Seen::Closed,
                    Err(real::error::TryRecvError::Lagged(n)) => Seen::Lagged(n),
                });
                mini_seen.push(match mini_rx.try_recv() {
                    Ok(v) => Seen::Value(v),
                    Err(TryRecvError::Empty) => Seen::Empty,
                    Err(TryRecvError::Closed) => Seen::Closed,
                    Err(TryRecvError::Lagged(n)) => Seen::Lagged(n),
                });
            }
        }
        drop(real_tx);
        drop(mini_tx);
        for _ in 0..6 {
            real_seen.push(match real_rx.try_recv() {
                Ok(v) => Seen::Value(v),
                Err(real::error::TryRecvError::Empty) => Seen::Empty,
                Err(real::error::TryRecvError::Closed) => Seen::Closed,
                Err(real::error::TryRecvError::Lagged(n)) => Seen::Lagged(n),
            });
            mini_seen.push(match mini_rx.try_recv() {
                Ok(v) => Seen::Value(v),
                Err(TryRecvError::Empty) => Seen::Empty,
                Err(TryRecvError::Closed) => Seen::Closed,
                Err(TryRecvError::Lagged(n)) => Seen::Lagged(n),
            });
        }
        assert_eq!(mini_seen, real_seen);
        assert!(real_seen.iter().any(|s| matches!(s, Seen::Lagged(_))));
    }
}
