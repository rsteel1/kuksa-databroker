// c24_e09_review_waker_storage.rs
//
// CONCEPT: code review — this file COMPILES, but has 2 planted bugs typical
//          of chapter 24 (a hand-rolled future that never stores the waker
//          it's given, and the other side that sets the ready flag but
//          never calls `wake()` on it).
// GUIDE:   chapters/24-async-internals/guide.md  (section "Wakers: how Pending ends")
// DATABROKER: databroker/src/broker.rs — `tokio::time::interval` in
//             `start_housekeeping_task` is a leaf future that parks its
//             waker with the timer driver and relies on it being woken;
//             a leaf future that drops its waker on the floor is polled
//             once, returns `Pending`, and is never scheduled again.
// EXPECT:  tests fail: a task waiting on the latch never completes, even
//          after the latch is opened.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`. You may
//   add fields to `Shared`.
//   Do not edit the tests, and do not change any function's signature.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Default)]
struct Shared {
    open: bool,
    // BUG 1 (fix): `poll` never had anywhere to put the waker it was
    // given, so `open()` had nothing to wake.
    waker: Option<Waker>,
}

/// The waiting side: a future that completes once the matching
/// `LatchOpener` calls `open()`.
struct Latch {
    shared: Arc<Mutex<Shared>>,
}

/// The signalling side.
struct LatchOpener {
    shared: Arc<Mutex<Shared>>,
}

fn latch_pair() -> (Latch, LatchOpener) {
    let shared = Arc::new(Mutex::new(Shared::default()));
    (
        Latch {
            shared: shared.clone(),
        },
        LatchOpener { shared },
    )
}

impl LatchOpener {
    fn open(&self) {
        let mut guard = self.shared.lock().expect("not poisoned");
        guard.open = true;
        // BUG 2: setting the flag isn't enough — nothing re-polls the
        // waiting task unless its stored waker is actually woken.
        if let Some(waker) = guard.waker.take() {
            waker.wake();
        }
    }
}

impl Future for Latch {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut guard = self.shared.lock().expect("not poisoned");
        if guard.open {
            Poll::Ready(())
        } else {
            // BUG 1 (fix): store the current waker every time — the
            // executor is free to poll with a different one (e.g. after
            // moving the task to another worker), so the latest one must
            // always win.
            guard.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn opening_the_latch_wakes_the_waiting_task() {
        let (latch, opener) = latch_pair();
        let handle = tokio::spawn(latch);
        tokio::task::yield_now().await;
        assert!(!handle.is_finished(), "latch starts closed");

        opener.open();
        // Give a woken task a few chances to run.
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }

        assert!(
            handle.is_finished(),
            "opening the latch should wake the task waiting on it"
        );
        handle.await.unwrap();
    }
}
