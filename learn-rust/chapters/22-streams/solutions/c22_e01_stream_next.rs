// c22_e01_stream_next.rs
//
// CONCEPT: a `Stream` is an async iterator; `StreamExt` gives it `next`,
//          `map`, `filter`, `take`, `collect`, ...
// GUIDE:   chapters/22-streams/guide.md  (section "Streams are async iterators")
// DATABROKER: databroker/src/broker.rs — `use tokio_stream::{Stream, StreamExt};`
//             and `subscribe` returns `impl Stream<Item = Option<EntryUpdates>>`
// EXPECT:  tests fail: `todo!()` panics
//
// `Iterator::next()` returns `Option<Item>` immediately. `Stream::next()`
// returns a future of `Option<Item>`: you `.await` it, and `None` means the
// stream has ended. Most adaptors you know from iterators exist on streams.
//
// Note: there are two `StreamExt` traits (tokio_stream's and futures').
// The databroker uses `tokio_stream::StreamExt`; so do we. Importing both in
// the same scope causes "multiple applicable items" errors.
//
// YOUR TASK
//   Implement `sum_first` with a `while let` + `.next().await` loop, and
//   `speeds_over` with stream adaptors (no loop).
//   Do not edit the tests.

use tokio_stream::{Stream, StreamExt};

/// Sum the first `n` items (or all items if the stream is shorter).
/// `Unpin` lets us call `.next()` on the stream in place — see the guide.
async fn sum_first(mut stream: impl Stream<Item = u32> + Unpin, n: usize) -> u32 {
    let mut sum = 0;
    let mut taken = 0;
    while taken < n {
        match stream.next().await {
            Some(value) => sum += value,
            None => break,
        }
        taken += 1;
    }
    sum
}

/// Keep only speeds strictly above `limit`, formatted as "Vehicle.Speed=<v>".
fn speeds_over(stream: impl Stream<Item = u32>, limit: u32) -> impl Stream<Item = String> {
    stream
        .filter(move |&speed| speed > limit)
        .map(|speed| format!("Vehicle.Speed={speed}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sums_prefix() {
        assert_eq!(sum_first(tokio_stream::iter(vec![1, 2, 3, 4]), 3).await, 6);
        assert_eq!(sum_first(tokio_stream::iter(vec![5]), 10).await, 5);
        assert_eq!(sum_first(tokio_stream::iter(Vec::new()), 3).await, 0);
    }

    #[tokio::test]
    async fn filters_and_formats() {
        let out: Vec<String> = speeds_over(tokio_stream::iter(vec![30, 130, 90, 140]), 100)
            .collect()
            .await;
        assert_eq!(out, vec!["Vehicle.Speed=130", "Vehicle.Speed=140"]);
    }
}
