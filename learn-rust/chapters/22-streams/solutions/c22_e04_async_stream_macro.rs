// c22_e04_async_stream_macro.rs
//
// CONCEPT: writing a stream as straight-line code with `async_stream::stream!`
//          and `try_stream!` (`yield` values, `?` for errors).
// GUIDE:   chapters/22-streams/guide.md  (section "Writing streams by hand: async-stream")
// DATABROKER: databroker/src/grpc/kuksa_val_v1/val.rs — the v1 `subscribe`
//             wraps the subscription stream in `stream! { loop { tokio::select! {
//             item = s.next() => ..., _ = shutdown_rx.recv() => { yield
//             Err(Status::unavailable("Databroker shutting down")); break; } } } }`
// EXPECT:  tests fail: `todo!()` panics
//
// Adaptors (`map`, `filter`) are great for transforming an existing stream.
// When a stream needs its own loop, state, or timing, `stream! { ... }` lets
// you write it like a normal async function that `yield`s items.
//
// YOUR TASK
//   Implement `countdown` (sleep `every`, then yield; from `from` down to 1),
//   `parse_all` (yield each parsed number; stop with the error on the
//   first input that isn't a number — use `try_stream!` and `?`), and
//   `until_shutdown` (forward every item as `Ok`, but on shutdown yield one
//   final `Err("Databroker shutting down")` and end — the v1 pattern).
//   Do not edit the tests.

use async_stream::{stream, try_stream};
use std::num::ParseIntError;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::{Stream, StreamExt};

fn countdown(from: u32, every: Duration) -> impl Stream<Item = u32> {
    stream! {
        for n in (1..=from).rev() {
            tokio::time::sleep(every).await;
            yield n;
        }
    }
}

fn parse_all(inputs: Vec<&'static str>) -> impl Stream<Item = Result<i32, ParseIntError>> {
    try_stream! {
        for input in inputs {
            let value: i32 = input.parse()?;
            yield value;
        }
    }
}

fn until_shutdown(
    updates: impl Stream<Item = String> + Send + 'static,
    mut shutdown: broadcast::Receiver<()>,
) -> impl Stream<Item = Result<String, String>> {
    stream! {
        // WHY `Box::pin`: `next()` needs an `Unpin` stream; pinning it on the
        // heap makes any stream `Unpin`.
        let mut updates = Box::pin(updates);
        loop {
            tokio::select! {
                item = updates.next() => match item {
                    Some(update) => yield Ok(update),
                    None => break,
                },
                _ = shutdown.recv() => {
                    yield Err("Databroker shutting down".to_string());
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use tokio::time::Instant;
    use tokio_stream::wrappers::ReceiverStream;

    #[tokio::test(start_paused = true)]
    async fn counts_down_with_timing() {
        let start = Instant::now();
        let values: Vec<u32> = countdown(3, Duration::from_millis(100)).collect().await;
        assert_eq!(values, vec![3, 2, 1]);
        assert_eq!(start.elapsed(), Duration::from_millis(300));
    }

    #[tokio::test]
    async fn stops_at_first_error() {
        let results: Vec<_> = parse_all(vec!["1", "2", "x", "4"]).collect().await;
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Ok(1));
        assert_eq!(results[1], Ok(2));
        assert!(results[2].is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_ends_stream_with_error() {
        let (tx, rx) = mpsc::channel(4);
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let mut stream = Box::pin(until_shutdown(ReceiverStream::new(rx), shutdown_rx));

        tx.send("speed=10".to_string()).await.unwrap();
        assert_eq!(stream.next().await, Some(Ok("speed=10".to_string())));
        shutdown_tx.send(()).unwrap();
        assert_eq!(
            stream.next().await,
            Some(Err("Databroker shutting down".to_string()))
        );
        assert_eq!(stream.next().await, None);
    }

    #[tokio::test]
    async fn upstream_end_ends_stream_cleanly() {
        let (_shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let updates = tokio_stream::iter(vec!["a".to_string()]);
        let out: Vec<_> = until_shutdown(updates, shutdown_rx).collect().await;
        assert_eq!(out, vec![Ok("a".to_string())]);
    }

    #[tokio::test]
    async fn all_good() {
        let results: Vec<_> = parse_all(vec!["10", "-3"]).collect().await;
        assert_eq!(results, vec![Ok(10), Ok(-3)]);
    }
}
