// c22_e07_review_unpolled_streams.rs
//
// CONCEPT: code review — this file COMPILES, but has 2 planted bugs typical
//          of chapter 22 (a stream only ever polled once instead of drained
//          in a loop, and a stream built inside a block that drops it
//          before it's ever polled at all).
// GUIDE:   chapters/22-streams/guide.md  (section "Channels as streams")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `open_provider_stream`
//             returns a `ReceiverStream` that a background task feeds; a
//             stream that's only polled once, or dropped before its first
//             poll, silently loses every update sent after that point.
// EXPECT:  tests fail: only the first matching value is ever seen, and a
//          drained stream comes back empty even though values were sent.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`.
//   Do not edit the tests, and do not change any function's signature.

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};

/// Counts how many values in the stream are strictly positive.
async fn count_positive(mut source: impl Stream<Item = i32> + Unpin) -> usize {
    let mut count = 0;
    // BUG 1: `if let` polls the stream exactly once and then drops it —
    // every value after the first was never even looked at. `while let`
    // keeps polling until the stream ends.
    while let Some(value) = source.next().await {
        if value > 0 {
            count += 1;
        }
    }
    count
}

/// Collects every value sent on `rx` into a `Vec`, in order.
async fn drain_into(rx: mpsc::Receiver<i32>) -> Vec<i32> {
    let mut items = Vec::new();
    // BUG 2: the stream was built and then immediately dropped at the end
    // of this block, without a single `.next().await` — it never got
    // polled, so it never received anything the sender sent. Keep it
    // alive across the loop that actually drains it.
    let mut stream = ReceiverStream::new(rx);
    while let Some(value) = stream.next().await {
        items.push(value);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn count_positive_checks_every_value() {
        let source = tokio_stream::iter(vec![1, -2, 3, -4, 5]);
        assert_eq!(count_positive(source).await, 3);
    }

    #[tokio::test]
    async fn drain_into_collects_everything_sent() {
        let (tx, rx) = mpsc::channel(8);
        for v in [10, 20, 30] {
            tx.send(v).await.unwrap();
        }
        drop(tx);
        assert_eq!(drain_into(rx).await, vec![10, 20, 30]);
    }
}
