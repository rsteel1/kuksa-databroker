// c24_e04_pin_and_unpin.rs
//
// CONCEPT: why futures are pinned, `Unpin`, `Pin::new` vs `pin!`/`Box::pin`,
//          and pin projection with `pin-project-lite` for wrapper futures.
// GUIDE:   chapters/24-async-internals/guide.md  (section "Pin and Unpin")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `type SubscribeStream =
//             Pin<Box<dyn Stream<...> + Send + Sync + 'static>>` (line 369):
//             `Box::pin` is how the databroker gets a pinned, nameable type.
//             Tokio's own `Sleep` is a `pin_project!` struct
//             (~/.cargo/registry/src/index.crates.io-*/tokio-1.53.1/src/time/sleep.rs:132).
// EXPECT:  compile error E0277 (`F` cannot be unpinned) in `WithPollCount::poll`
//
// An `async` block that keeps a reference to its own local across an
// `.await` is self-referential: moving it in memory would leave the reference
// dangling. So such futures are `!Unpin`, and may only be polled through a
// `Pin<&mut F>` — a promise that the value will never move again.
//
// A wrapper future (timeout, instrumentation, `map`) owns an inner future and
// must forward `poll` to it. It needs `Pin<&mut Inner>` from its own
// `Pin<&mut Self>` — that conversion is called *pin projection*.
// `pin_project_lite::pin_project!` generates it safely: fields marked
// `#[pin]` project to `Pin<&mut T>`, the others to plain `&mut T`.
//
// YOUR TASK
//   1. Make `WithPollCount<F>` work for ANY future `F`, including `!Unpin`
//      ones: wrap the struct in `pin_project! { ... }`, mark `inner` as
//      `#[pin]`, and use `self.project()` in `poll`.
//   2. Implement `poll_once`: pin the future on the stack with
//      `std::pin::pin!` and poll it a single time with `Waker::noop()`.
//   Do not edit the tests.

// I AM NOT DONE

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

/// Wraps a future and reports how many times it was polled.
struct WithPollCount<F> {
    inner: F,
    polls: u32,
}

impl<F: Future> Future for WithPollCount<F> {
    type Output = (F::Output, u32);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.polls += 1;
        match Pin::new(&mut self.inner).poll(cx) {
            Poll::Ready(out) => Poll::Ready((out, self.polls)),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn with_poll_count<F: Future>(inner: F) -> WithPollCount<F> {
    WithPollCount { inner, polls: 0 }
}

/// Polls `fut` exactly once. Works for `!Unpin` futures too.
fn poll_once<F: Future>(fut: F) -> Poll<F::Output> {
    todo!()
}

/// Returns `Pending` the first time, then `Ready` (wakes itself).
struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn counts_polls_of_a_self_referential_async_block() {
        let fut = async {
            let readings = String::from("abc"); // heap data owned by the future
            let first = &readings; // reference to our own local...
            YieldOnce(false).await; // ...kept alive across an await => !Unpin
            first.len() as i32 + 10
        };
        // `fut` is !Unpin: `Pin::new(&mut fut)` would not compile here.
        let (out, polls) = with_poll_count(fut).await;
        assert_eq!(out, 13);
        assert_eq!(polls, 2);
    }

    #[tokio::test]
    async fn works_for_unpin_futures_too() {
        let (out, polls) = with_poll_count(std::future::ready("ok")).await;
        assert_eq!((out, polls), ("ok", 1));
    }

    #[test]
    fn poll_once_on_ready_and_pending_futures() {
        assert_eq!(poll_once(async { 5 }), Poll::Ready(5));
        let pending = async {
            YieldOnce(false).await;
            1
        };
        assert_eq!(poll_once(pending), Poll::Pending);
    }

    #[tokio::test]
    async fn boxed_pinned_futures_can_be_stored_and_moved() {
        // `Box::pin` puts the future on the heap: the Box can move around
        // freely while the future itself stays put.
        let mut futures: Vec<Pin<Box<dyn Future<Output = u32> + Send>>> = Vec::new();
        futures.push(Box::pin(async { 1 }));
        futures.push(Box::pin(async {
            YieldOnce(false).await;
            2
        }));
        let mut total = 0;
        for fut in futures {
            total += fut.await;
        }
        assert_eq!(total, 3);
    }
}
