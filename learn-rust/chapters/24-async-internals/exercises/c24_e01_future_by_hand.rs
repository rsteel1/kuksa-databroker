// c24_e01_future_by_hand.rs
//
// CONCEPT: the `Future` trait itself — `poll`, `Poll::Ready`/`Poll::Pending`,
//          `Context` and the waker contract; polling a future by hand.
// GUIDE:   chapters/24-async-internals/guide.md  (section "The Future trait, by hand")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — every `.await` in
//             `open_provider_stream` (line 909) compiles down to repeated calls
//             to `poll` on futures like the ones you write here.
// EXPECT:  tests fail: `todo!()` panics
//
// Chapter 20 said "you will almost never call `poll` yourself". This chapter
// is the exception: to read library code (tokio, tower, tonic, hyper) you must
// know exactly what `poll` promises. Two rules:
//   1. Return `Poll::Ready(value)` exactly once, when the work is done.
//   2. If you return `Poll::Pending`, you MUST have arranged for
//      `cx.waker()` to be woken later — otherwise nobody polls you again and
//      the task hangs forever.
//
// YOUR TASK
//   1. Implement `Future for Ready<T>`: complete on the first poll.
//   2. Implement `Future for Countdown`: every poll increments `polls`; while
//      `remaining > 0` decrement it, wake yourself (`cx.waker().wake_by_ref()`)
//      and return `Pending`; when it reaches 0 return `Ready("liftoff")`.
//   3. Implement `poll_by_hand`, which drives any `Unpin` future to completion
//      using `Waker::noop()` and returns (output, number of polls).
//   Do not edit the tests.

// I AM NOT DONE

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

/// A future that is immediately ready with a value (like `std::future::ready`).
struct Ready<T> {
    value: Option<T>,
}

impl<T> Ready<T> {
    fn new(value: T) -> Self {
        Ready { value: Some(value) }
    }
}

// `T: Unpin` lets us use `self.get_mut()`: moving out of a pinned value is
// only allowed when the type promises it doesn't care about being moved.
impl<T: Unpin> Future for Ready<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        todo!()
    }
}

/// Needs `remaining` extra polls before it completes.
struct Countdown {
    remaining: u32,
    polls: u32,
}

impl Countdown {
    fn new(remaining: u32) -> Self {
        Countdown {
            remaining,
            polls: 0,
        }
    }
}

impl Future for Countdown {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<&'static str> {
        todo!()
    }
}

/// Polls `fut` in a loop with a no-op waker until it is ready.
/// Returns the output and how many times `poll` was called.
///
/// A real executor would *sleep* until woken instead of spinning; the no-op
/// waker is only acceptable because our futures wake themselves immediately.
fn poll_by_hand<F: Future + Unpin>(mut fut: F) -> (F::Output, u32) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_completes_on_first_poll() {
        assert_eq!(poll_by_hand(Ready::new(42)), (42, 1));
    }

    #[test]
    fn countdown_needs_n_plus_one_polls() {
        assert_eq!(poll_by_hand(Countdown::new(0)), ("liftoff", 1));
        assert_eq!(poll_by_hand(Countdown::new(3)), ("liftoff", 4));
    }

    #[test]
    fn countdown_counts_its_own_polls() {
        let mut cd = Countdown::new(2);
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(Pin::new(&mut cd).poll(&mut cx), Poll::Pending);
        assert_eq!(Pin::new(&mut cd).poll(&mut cx), Poll::Pending);
        assert_eq!(Pin::new(&mut cd).poll(&mut cx), Poll::Ready("liftoff"));
        assert_eq!(cd.polls, 3);
    }

    #[tokio::test]
    async fn hand_written_futures_work_with_await() {
        // `.await` on a real runtime: Tokio calls `poll` for us and relies on
        // the wake-up in `Countdown::poll` to schedule the next poll.
        assert_eq!(Ready::new("hi").await, "hi");
        let mut cd = Countdown::new(5);
        // `&mut F` is itself a future when `F: Unpin`, so we keep `cd` afterwards.
        assert_eq!((&mut cd).await, "liftoff");
        assert_eq!(cd.polls, 6);
    }
}
