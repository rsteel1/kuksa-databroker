// c22_e03_broadcast_stream_lagged_to_none.rs
//
// CONCEPT: `BroadcastStream` + `map` + `filter_map` — the exact pipeline
//          behind a databroker subscription.
// GUIDE:   chapters/22-streams/guide.md  (section "BroadcastStream and the subscribe pipeline")
// DATABROKER: databroker/src/broker.rs — end of `AuthorizedAccess::subscribe`:
//             `BroadcastStream::new(receiver).map(move |result| match result {
//             Ok(message) => message, Err(BroadcastStreamRecvError::Lagged(n))
//             => { warn!(...); counter.add(n, ...); None } })`;
//             grpc/kuksa_val_v2/val.rs — `convert_to_proto_stream` then drops
//             the `None`s with `filter_map`
// EXPECT:  tests fail: `todo!()` panics
//
// In the broker, stream items are `Option<EntryUpdates>`: `Some(update)` is a
// real notification, `None` means "something went wrong — nothing to send".
// A lagging subscriber doesn't kill its stream; it just skips ahead.
//
// YOUR TASK
//   1. `subscribe`: subscribe to `tx`, wrap in `BroadcastStream`, map
//      `Ok(v)` to `Some(v)` and `Lagged(n)` to `None` while adding `n` to
//      the shared `lagged` counter (think: the OpenTelemetry drop counter).
//   2. `only_values`: drop the `None`s.
//   Do not edit the tests.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

fn subscribe(
    tx: &broadcast::Sender<i32>,
    lagged: Arc<AtomicU64>,
) -> impl Stream<Item = Option<i32>> {
    BroadcastStream::new(tx.subscribe()).map(move |result| match result {
        Ok(value) => Some(value),
        Err(BroadcastStreamRecvError::Lagged(n)) => {
            lagged.fetch_add(n, Ordering::Relaxed);
            None
        }
    })
}

fn only_values(stream: impl Stream<Item = Option<i32>>) -> impl Stream<Item = i32> {
    // WHY: `filter_map` keeps `Some(v)` as `v` and silently drops `None`.
    stream.filter_map(|item| item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lag_becomes_none_and_is_counted() {
        let (tx, rx) = broadcast::channel(1);
        let lagged = Arc::new(AtomicU64::new(0));
        let stream = subscribe(&tx, lagged.clone());
        for v in 1..=3 {
            tx.send(v).unwrap();
        }
        drop(tx);
        drop(rx);
        let items: Vec<Option<i32>> = stream.collect().await;
        assert_eq!(items, vec![None, Some(3)]);
        assert_eq!(lagged.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn pipeline_yields_only_real_updates() {
        let (tx, _) = broadcast::channel(2);
        let lagged = Arc::new(AtomicU64::new(0));
        let stream = only_values(subscribe(&tx, lagged.clone()));
        for v in 1..=5 {
            tx.send(v).unwrap();
        }
        drop(tx);
        assert_eq!(stream.collect::<Vec<_>>().await, vec![4, 5]);
        assert_eq!(lagged.load(Ordering::Relaxed), 3);
    }
}
