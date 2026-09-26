// c25_e02_condvar.rs
//
// CONCEPT: `Condvar` — sleeping until a condition on Mutex-protected state
//          becomes true; `wait_while`, `wait_timeout_while`, `notify_one`
//          vs `notify_all`, spurious wake-ups.
// GUIDE:   chapters/25-concurrency-in-depth/guide.md  (section "Condvar: waiting for a condition")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — the bounded
//             `mpsc::channel(10)` in `open_provider_stream` (line 929) is an
//             async bounded queue: `send().await` waits while it's full. You
//             build the blocking (thread) version of the same idea here.
//             Tokio's own thread parker is a Mutex + Condvar
//             (~/.cargo/registry/src/index.crates.io-*/tokio-1.53.1/src/runtime/park.rs:24).
// EXPECT:  tests fail: `todo!()` panics
//
// A Condvar is always paired with a Mutex. The pattern is:
//     let guard = mutex.lock();
//     let guard = condvar.wait_while(guard, |state| !ready(state));  // sleeps
//     ...use state...                                                  // lock held
// `wait` atomically releases the lock while sleeping and re-acquires it on
// wake-up. Wake-ups can be spurious, so ALWAYS re-check the condition
// (`wait_while` does the loop for you).
//
// YOUR TASK
//   Implement `BoundedQueue<T>`, a blocking multi-producer multi-consumer
//   queue with a fixed capacity:
//     * `push` blocks while the queue is full;
//     * `pop` blocks while it is empty, and returns `None` once the queue is
//       closed AND drained;
//     * `pop_timeout` gives up after `timeout` and returns `None`;
//     * `close` wakes every waiter; `push` after close returns `Err(item)`.
//   Use one Mutex and two Condvars (`not_full`, `not_empty`).
//   Do not edit the tests.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

struct State<T> {
    items: VecDeque<T>,
    closed: bool,
}

struct BoundedQueue<T> {
    capacity: usize,
    state: Mutex<State<T>>,
    not_full: Condvar,
    not_empty: Condvar,
}

impl<T> BoundedQueue<T> {
    fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be at least 1");
        BoundedQueue {
            capacity,
            state: Mutex::new(State {
                items: VecDeque::with_capacity(capacity),
                closed: false,
            }),
            not_full: Condvar::new(),
            not_empty: Condvar::new(),
        }
    }

    /// Blocks while full. Returns `Err(item)` if the queue is (or becomes) closed.
    fn push(&self, item: T) -> Result<(), T> {
        let guard = self.state.lock().expect("queue poisoned");
        let mut state = self
            .not_full
            .wait_while(guard, |s| !s.closed && s.items.len() >= self.capacity)
            .expect("queue poisoned");
        if state.closed {
            return Err(item);
        }
        state.items.push_back(item);
        // WHY: notify_one is enough — one item can satisfy one consumer.
        self.not_empty.notify_one();
        Ok(())
    }

    /// Blocks while empty. `None` once closed and drained.
    fn pop(&self) -> Option<T> {
        let guard = self.state.lock().expect("queue poisoned");
        let mut state = self
            .not_empty
            .wait_while(guard, |s| !s.closed && s.items.is_empty())
            .expect("queue poisoned");
        let item = state.items.pop_front();
        if item.is_some() {
            self.not_full.notify_one();
        }
        item
    }

    /// Like `pop`, but returns `None` if nothing arrives within `timeout`.
    fn pop_timeout(&self, timeout: Duration) -> Option<T> {
        let guard = self.state.lock().expect("queue poisoned");
        let (mut state, _timed_out) = self
            .not_empty
            .wait_timeout_while(guard, timeout, |s| !s.closed && s.items.is_empty())
            .expect("queue poisoned");
        let item = state.items.pop_front();
        if item.is_some() {
            self.not_full.notify_one();
        }
        item
    }

    fn close(&self) {
        self.state.lock().expect("queue poisoned").closed = true;
        // WHY: notify_all — EVERY blocked producer and consumer must re-check
        // and see `closed`, otherwise some would sleep forever.
        self.not_full.notify_all();
        self.not_empty.notify_all();
    }

    fn len(&self) -> usize {
        self.state.lock().expect("queue poisoned").items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn fifo_single_thread() {
        let q = BoundedQueue::new(3);
        q.push(1).unwrap();
        q.push(2).unwrap();
        assert_eq!(q.pop(), Some(1));
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn pop_timeout_on_empty_queue_returns_none() {
        let q: BoundedQueue<u32> = BoundedQueue::new(1);
        assert_eq!(q.pop_timeout(Duration::from_millis(20)), None);
    }

    #[test]
    fn close_drains_then_ends_and_rejects_pushes() {
        let q = BoundedQueue::new(4);
        q.push("a").unwrap();
        q.close();
        assert_eq!(q.push("b"), Err("b"));
        assert_eq!(q.pop(), Some("a"));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn producer_blocks_while_full_until_consumer_pops() {
        let q = BoundedQueue::new(1);
        q.push(1).unwrap();
        thread::scope(|s| {
            let producer = s.spawn(|| q.push(2));
            // The producer can't finish while the queue is full. We can't
            // observe "blocked" directly, but we can prove the item isn't in yet:
            assert_eq!(q.pop_timeout(Duration::from_secs(2)), Some(1));
            assert_eq!(producer.join().unwrap(), Ok(()));
            assert_eq!(q.pop_timeout(Duration::from_secs(2)), Some(2));
        });
    }

    #[test]
    fn close_wakes_blocked_consumers() {
        let q: BoundedQueue<u32> = BoundedQueue::new(2);
        let started = Barrier::new(4);
        thread::scope(|s| {
            let consumers: Vec<_> = (0..3)
                .map(|_| {
                    s.spawn(|| {
                        started.wait();
                        q.pop()
                    })
                })
                .collect();
            started.wait();
            q.close();
            for c in consumers {
                assert_eq!(c.join().unwrap(), None, "woken by close");
            }
        });
    }

    #[test]
    fn many_producers_many_consumers_lose_nothing() {
        let q = BoundedQueue::new(4);
        let total: u64 = thread::scope(|s| {
            let consumers: Vec<_> = (0..3)
                .map(|_| {
                    s.spawn(|| {
                        let mut sum = 0u64;
                        while let Some(v) = q.pop() {
                            sum += v;
                        }
                        sum
                    })
                })
                .collect();
            let producers: Vec<_> = (0..4u64)
                .map(|p| {
                    let q = &q;
                    s.spawn(move || {
                        for i in 0..250u64 {
                            q.push(p * 1000 + i).unwrap();
                        }
                    })
                })
                .collect();
            for p in producers {
                p.join().unwrap();
            }
            q.close();
            consumers.into_iter().map(|c| c.join().unwrap()).sum()
        });
        let expected: u64 = (0..4u64)
            .flat_map(|p| (0..250u64).map(move |i| p * 1000 + i))
            .sum();
        assert_eq!(total, expected);
    }
}
