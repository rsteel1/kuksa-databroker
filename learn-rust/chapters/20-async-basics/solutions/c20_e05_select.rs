// c20_e05_select.rs
//
// CONCEPT: `tokio::select!` — wait for whichever of several futures is
//          ready first; the losing branches are cancelled (dropped).
// GUIDE:   chapters/20-async-basics/guide.md  (section "select! and cancellation safety")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `open_provider_stream`
//             loops on `select! { message = stream.message() => ...,
//             _ = shutdown_trigger.recv() => break }`
// EXPECT:  tests fail: `todo!()` panics
//
// Every provider connection in the databroker is a task running this loop:
// "handle the next message from the provider, unless the broker is shutting
// down". You'll write the same loop.
//
// Only put *cancellation-safe* futures in a select! loop: if the other branch
// wins, the future is dropped, and it must not have half-consumed a message.
// `mpsc::Receiver::recv` and `broadcast::Receiver::recv` are cancellation safe.
//
// YOUR TASK
//   Implement `provider_loop`: collect every message received on `messages`
//   until either the channel is closed (`recv()` returns `None`) or a
//   shutdown signal arrives on `shutdown`. Return what was collected.
//   Do not edit the tests.

use tokio::select;
use tokio::sync::{broadcast, mpsc};

async fn provider_loop(
    mut messages: mpsc::Receiver<String>,
    mut shutdown: broadcast::Receiver<()>,
) -> Vec<String> {
    let mut collected = Vec::new();
    loop {
        select! {
            message = messages.recv() => match message {
                Some(message) => collected.push(message),
                None => break, // every sender dropped: the provider disconnected
            },
            _ = shutdown.recv() => break,
        }
    }
    collected
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test(start_paused = true)]
    async fn stops_on_shutdown() {
        let (tx, rx) = mpsc::channel(8);
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let task = tokio::spawn(provider_loop(rx, shutdown_rx));

        tx.send("a".to_string()).await.unwrap();
        sleep(Duration::from_millis(10)).await;
        tx.send("b".to_string()).await.unwrap();
        sleep(Duration::from_millis(10)).await;
        shutdown_tx.send(()).unwrap();
        sleep(Duration::from_millis(10)).await;
        // The loop has ended; this message is never collected.
        let _ = tx.send("c".to_string()).await;

        assert_eq!(task.await.unwrap(), vec!["a", "b"]);
    }

    #[tokio::test(start_paused = true)]
    async fn stops_when_sender_is_dropped() {
        let (tx, rx) = mpsc::channel(8);
        let (_shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let task = tokio::spawn(provider_loop(rx, shutdown_rx));

        tx.send("only".to_string()).await.unwrap();
        drop(tx);

        assert_eq!(task.await.unwrap(), vec!["only"]);
    }
}
