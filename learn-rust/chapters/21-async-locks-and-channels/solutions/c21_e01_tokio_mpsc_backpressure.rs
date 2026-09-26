// c21_e01_tokio_mpsc_backpressure.rs
//
// CONCEPT: bounded `tokio::sync::mpsc` channels, backpressure, `try_send`,
//          and "channel closed" as a disconnect signal.
// GUIDE:   chapters/21-async-locks-and-channels/guide.md  (section "mpsc: many producers, one consumer")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `open_provider_stream`
//             creates `mpsc::channel(10)` for responses to the provider, and
//             `Provider::is_available` is `!self.sender.is_closed()`
// EXPECT:  tests fail: `todo!()` panics
//
// A bounded channel has room for N messages. When it's full,
// `send(x).await` WAITS until the consumer makes room — that's backpressure:
// a slow consumer automatically slows the producer down instead of letting
// memory grow forever. `try_send` never waits; it tells you "full" instead.
//
// YOUR TASK
//   Implement `publish` (non-blocking, report what happened), `drain`, and
//   `consumer_gone`.
//   Do not edit the tests.

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

#[derive(Debug, PartialEq)]
enum PublishOutcome {
    Sent,
    /// Channel is at capacity; the value was NOT sent.
    Full,
    /// The receiver was dropped: nobody is listening any more.
    Closed,
}

/// Try to send without waiting. Map the three possible results.
fn publish(tx: &mpsc::Sender<i32>, value: i32) -> PublishOutcome {
    match tx.try_send(value) {
        Ok(()) => PublishOutcome::Sent,
        Err(TrySendError::Full(_value)) => PublishOutcome::Full,
        Err(TrySendError::Closed(_value)) => PublishOutcome::Closed,
    }
}

/// Receive until every sender has been dropped; return all values in order.
async fn drain(mut rx: mpsc::Receiver<i32>) -> Vec<i32> {
    let mut values = Vec::new();
    while let Some(value) = rx.recv().await {
        values.push(value);
    }
    values
}

/// True if the receiving side is gone (like `Provider::is_available`, inverted).
fn consumer_gone(tx: &mpsc::Sender<i32>) -> bool {
    tx.is_closed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn full_channel_rejects_try_send() {
        let (tx, rx) = mpsc::channel(2);
        assert_eq!(publish(&tx, 1), PublishOutcome::Sent);
        assert_eq!(publish(&tx, 2), PublishOutcome::Sent);
        assert_eq!(publish(&tx, 3), PublishOutcome::Full);
        drop(tx);
        assert_eq!(drain(rx).await, vec![1, 2]);
    }

    #[tokio::test]
    async fn closed_when_receiver_dropped() {
        let (tx, rx) = mpsc::channel(2);
        assert!(!consumer_gone(&tx));
        drop(rx);
        assert!(consumer_gone(&tx));
        assert_eq!(publish(&tx, 1), PublishOutcome::Closed);
    }

    #[tokio::test(start_paused = true)]
    async fn send_await_waits_for_room() {
        let (tx, mut rx) = mpsc::channel(1);
        tx.send(1).await.unwrap(); // fills the channel
        let consumer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let first = rx.recv().await;
            (first, drain(rx).await)
        });
        let start = tokio::time::Instant::now();
        tx.send(2).await.unwrap(); // must wait until the consumer makes room
        assert_eq!(start.elapsed(), Duration::from_millis(500));
        drop(tx);
        assert_eq!(consumer.await.unwrap(), (Some(1), vec![2]));
    }
}
