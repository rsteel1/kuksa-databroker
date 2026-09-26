// c24_e02_waker_and_timer.rs
//
// CONCEPT: storing the `Waker` and waking it from somewhere else (another
//          thread, an I/O driver) — the heart of every leaf future.
// GUIDE:   chapters/24-async-internals/guide.md  (section "Wakers: how Pending ends")
// DATABROKER: databroker/src/broker.rs — `tokio::time::interval` in
//             `start_housekeeping_task` (line 2368) is a leaf future of exactly
//             this kind: it returns `Pending`, parks its waker in Tokio's timer
//             wheel, and the timer driver calls `wake()` when the tick is due.
// EXPECT:  tests fail: `todo!()` panics
//
// `Countdown` in e01 woke itself immediately — that's a busy loop. Real leaf
// futures (sockets, timers, channels) return `Pending`, hand their waker to
// whoever will produce the value, and are only polled again when that party
// calls `wake()`.
//
// Contract details that bite:
//   * The executor may poll you again with a DIFFERENT waker (the task moved
//     to another worker thread). Always store the latest one;
//     `Waker::will_wake` lets you skip the clone when it's the same.
//   * Wake-ups can be spurious: being polled does not mean you're ready.
//     Check your state every time.
//
// YOUR TASK
//   Implement `Future for Signal`: `Pending` until `SignalHandle::fire` has
//   been called, then `Ready(value)`. `fire` stores the value and wakes the
//   most recently stored waker (if any).
//   Do not edit the tests.

// I AM NOT DONE

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

#[derive(Default)]
struct Shared {
    value: Option<u32>,
    waker: Option<Waker>,
}

/// A future that completes when some other thread fires the matching handle
/// (a tiny oneshot channel).
struct Signal {
    shared: Arc<Mutex<Shared>>,
}

#[derive(Clone)]
struct SignalHandle {
    shared: Arc<Mutex<Shared>>,
}

fn signal() -> (Signal, SignalHandle) {
    let shared = Arc::new(Mutex::new(Shared::default()));
    (
        Signal {
            shared: shared.clone(),
        },
        SignalHandle { shared },
    )
}

impl SignalHandle {
    fn fire(&self, value: u32) {
        todo!()
    }
}

impl Future for Signal {
    type Output = u32;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        todo!()
    }
}

/// A waker that counts how often it was woken (implemented via `std::task::Wake`).
struct CountingWaker {
    wakes: AtomicUsize,
}

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counting_waker() -> (Arc<CountingWaker>, Waker) {
        let counter = Arc::new(CountingWaker {
            wakes: AtomicUsize::new(0),
        });
        let waker = Waker::from(counter.clone());
        (counter, waker)
    }

    #[test]
    fn pending_until_fired_then_ready() {
        let (mut sig, handle) = signal();
        let (counter, waker) = counting_waker();
        let mut cx = Context::from_waker(&waker);

        assert_eq!(Pin::new(&mut sig).poll(&mut cx), Poll::Pending);
        assert_eq!(counter.wakes.load(Ordering::SeqCst), 0);

        handle.fire(7);
        assert_eq!(
            counter.wakes.load(Ordering::SeqCst),
            1,
            "fire() must wake the stored waker exactly once"
        );
        assert_eq!(Pin::new(&mut sig).poll(&mut cx), Poll::Ready(7));
    }

    #[test]
    fn spurious_polls_stay_pending() {
        let (mut sig, _handle) = signal();
        let (_counter, waker) = counting_waker();
        let mut cx = Context::from_waker(&waker);
        for _ in 0..3 {
            assert_eq!(Pin::new(&mut sig).poll(&mut cx), Poll::Pending);
        }
    }

    #[test]
    fn the_latest_waker_is_the_one_woken() {
        let (mut sig, handle) = signal();
        let (first, first_waker) = counting_waker();
        let (second, second_waker) = counting_waker();

        let _ = Pin::new(&mut sig).poll(&mut Context::from_waker(&first_waker));
        // The task "moved": the executor polls with a different waker now.
        let _ = Pin::new(&mut sig).poll(&mut Context::from_waker(&second_waker));

        handle.fire(1);
        assert_eq!(first.wakes.load(Ordering::SeqCst), 0);
        assert_eq!(second.wakes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn firing_before_the_first_poll_is_fine() {
        let (mut sig, handle) = signal();
        handle.fire(3);
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(Pin::new(&mut sig).poll(&mut cx), Poll::Ready(3));
    }

    #[tokio::test]
    async fn woken_from_another_os_thread() {
        let (sig, handle) = signal();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            handle.fire(99);
        });
        // Tokio polls once, gets Pending, and only polls again after the
        // other thread calls `wake()`. A missing wake-up would hang here,
        // so we bound it.
        let value = tokio::time::timeout(Duration::from_secs(2), sig)
            .await
            .expect("the future was never woken");
        assert_eq!(value, 99);
    }
}
