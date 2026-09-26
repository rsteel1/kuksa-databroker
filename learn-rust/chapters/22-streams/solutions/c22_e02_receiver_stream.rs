// c22_e02_receiver_stream.rs
//
// CONCEPT: turning a channel receiver into a `Stream` with `ReceiverStream`,
//          fed by a spawned task.
// GUIDE:   chapters/22-streams/guide.md  (section "Channels as streams")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `open_provider_stream`
//             spawns a task that pushes into `response_stream_sender` and
//             immediately returns `ReceiverStream::new(response_stream_receiver)`
//             to tonic; `type OpenProviderStreamStream = ReceiverStream<...>`
// EXPECT:  tests fail: `todo!()` panics
//
// This is THE pattern for a server-streaming gRPC call: return a stream
// right away, and let a background task fill it. When the task ends (and
// drops its sender), the stream ends and tonic closes the response.
//
// YOUR TASK
//   Implement `spawn_producer` (spawn a task that sends every value, then
//   return the receiving side as a stream) and `as_updates`.
//   Do not edit the tests.

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};

fn spawn_producer(values: Vec<i32>) -> ReceiverStream<i32> {
    let (tx, rx) = mpsc::channel(4);
    tokio::spawn(async move {
        for value in values {
            // WHY: stop early if the consumer (e.g. the gRPC client) went away.
            if tx.send(value).await.is_err() {
                break;
            }
        }
        // `tx` is dropped here -> the stream ends.
    });
    ReceiverStream::new(rx)
}

/// Map each value to `Ok("update <v>")`, the shape tonic streams expect.
fn as_updates(stream: impl Stream<Item = i32>) -> impl Stream<Item = Result<String, String>> {
    stream.map(|value| Ok(format!("update {value}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stream_ends_when_producer_finishes() {
        // More values than the channel capacity: backpressure must not deadlock.
        let values: Vec<i32> = (1..=10).collect();
        let collected: Vec<i32> = spawn_producer(values.clone()).collect().await;
        assert_eq!(collected, values);
    }

    #[tokio::test]
    async fn maps_to_results() {
        let out: Vec<_> = as_updates(spawn_producer(vec![7, 8])).collect().await;
        assert_eq!(
            out,
            vec![Ok("update 7".to_string()), Ok("update 8".to_string())]
        );
    }

    #[tokio::test]
    async fn consumer_can_stop_early() {
        let first_two: Vec<i32> = spawn_producer((1..=1000).collect()).take(2).collect().await;
        assert_eq!(first_two, vec![1, 2]);
    }
}
