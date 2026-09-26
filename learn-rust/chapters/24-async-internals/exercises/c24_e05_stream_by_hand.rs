// c24_e05_stream_by_hand.rs
//
// CONCEPT: implementing `Stream::poll_next` directly — a source stream and
//          a pin-projected adapter — and seeing what `StreamExt::map` and
//          `async-stream` generate for you.
// GUIDE:   chapters/24-async-internals/guide.md  (section "Streams by hand")
// DATABROKER: databroker/src/broker.rs — `subscribe` (line 1665) builds
//             `BroadcastStream::new(receiver).map(|result| match result {
//             Ok(message) => message, Err(Lagged(n)) => { warn!(...); None } })`.
//             `LagToNone` below is that `.map(...)` written out by hand.
// EXPECT:  tests fail: `todo!()` panics
//
// `Stream` is `Future`'s sibling:
//
//     fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>)
//         -> Poll<Option<Self::Item>>;
//
//   Ready(Some(item))  -> here's the next item
//   Ready(None)        -> the stream is finished (don't poll again)
//   Pending            -> nothing yet; I've arranged a wake-up
//
// YOUR TASK
//   1. `Counter`: yields `start, start+1, ..., end` (inclusive), then `None`.
//      Also implement `size_hint` precisely.
//   2. `LagToNone<S>`: an adapter over a stream of `Result<T, Lagged>` that
//      yields `Some(t)` for `Ok(t)` and `None` for `Err(Lagged(_))`, counting
//      the total number of missed messages in `missed`. It must forward
//      `Pending` and end when the inner stream ends. Use `pin_project!`.
//   Do not edit the tests.

// I AM NOT DONE

use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_stream::Stream;

struct Counter {
    next: u32,
    end: u32,
}

impl Counter {
    fn new(start: u32, end: u32) -> Self {
        Counter { next: start, end }
    }
}

impl Stream for Counter {
    type Item = u32;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<u32>> {
        todo!()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        todo!()
    }
}

/// Stand-in for `BroadcastStreamRecvError::Lagged(n)`.
#[derive(Debug, PartialEq)]
struct Lagged(u64);

pin_project! {
    struct LagToNone<S> {
        #[pin]
        inner: S,
        missed: u64,
    }
}

impl<S> LagToNone<S> {
    fn new(inner: S) -> Self {
        LagToNone { inner, missed: 0 }
    }
}

impl<S, T> Stream for LagToNone<S>
where
    S: Stream<Item = Result<T, Lagged>>,
{
    type Item = Option<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Option<T>>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::ReceiverStream;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn counter_yields_inclusive_range() {
        let items: Vec<u32> = Counter::new(3, 6).collect().await;
        assert_eq!(items, vec![3, 4, 5, 6]);
        let empty: Vec<u32> = Counter::new(5, 4).collect().await;
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn counter_size_hint_is_exact_and_shrinks() {
        let mut c = Counter::new(1, 3);
        assert_eq!(c.size_hint(), (3, Some(3)));
        c.next().await;
        assert_eq!(c.size_hint(), (2, Some(2)));
        c.next().await;
        c.next().await;
        assert_eq!(c.size_hint(), (0, Some(0)));
        assert_eq!(c.next().await, None);
    }

    #[tokio::test]
    async fn lag_becomes_none_and_is_counted() {
        let inner = tokio_stream::iter(vec![Ok(1), Err(Lagged(4)), Ok(2), Err(Lagged(1))]);
        let mut s = LagToNone::new(inner);
        let mut out = Vec::new();
        while let Some(item) = s.next().await {
            out.push(item);
        }
        assert_eq!(out, vec![Some(1), None, Some(2), None]);
        assert_eq!(s.missed, 5);
    }

    #[tokio::test(start_paused = true)]
    async fn pending_is_forwarded_not_swallowed() {
        let (tx, rx) = mpsc::channel(4);
        let mut s = LagToNone::new(ReceiverStream::new(rx));
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            tx.send(Ok::<_, Lagged>("late")).await.unwrap();
        });
        // The first poll finds nothing: LagToNone must return Pending (not
        // `Ready(None)`, which would end the stream early).
        let item = tokio::time::timeout(Duration::from_secs(1), s.next())
            .await
            .expect("woken when the item arrives");
        assert_eq!(item, Some(Some("late")));
        assert_eq!(
            s.next().await,
            None,
            "stream ends when the sender is dropped"
        );
    }

    #[tokio::test]
    async fn same_thing_with_combinators() {
        // For comparison: what the databroker actually writes.
        let inner = tokio_stream::iter(vec![Ok(1), Err(Lagged(2))]);
        let mapped: Vec<Option<i32>> = inner.map(|r: Result<i32, Lagged>| r.ok()).collect().await;
        assert_eq!(mapped, vec![Some(1), None]);
    }
}
