// c21_e07_deadline_budget.rs
//
// CONCEPT: one deadline shared by several awaits ("time budget"), using
//          `Instant::saturating_duration_since`.
// GUIDE:   chapters/21-async-locks-and-channels/guide.md  (section "Deadlines across several steps")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs —
//             `Provider::get_signals_values_from_provider` computes
//             `let deadline = ... + Duration::from_secs(1)` and bounds EVERY
//             phase (lock, send, each receive) with
//             `deadline.saturating_duration_since(Instant::now())`
// EXPECT:  tests fail: `todo!()` panics
//
// Wrapping each step in its own `timeout(1s, ...)` would let a request take
// 1s per step. A deadline gives the WHOLE operation 1s: each step gets only
// what's left. `saturating_duration_since` returns 0 instead of panicking
// when the deadline has already passed.
//
// YOUR TASK
//   Implement `request_with_deadline`: send `request` on `tx` (this may have
//   to wait if the channel is full), then wait for the reply on `rx`. Both
//   steps together must fit into `budget`. Use `tokio::time::Instant`
//   (it follows the paused test clock) and `tokio::time::timeout`.
//   Do not edit the tests.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{timeout, Instant};

#[derive(Debug, PartialEq)]
enum RequestError {
    DeadlineExceeded,
    Disconnected,
}

async fn request_with_deadline(
    tx: &mpsc::Sender<u32>,
    rx: &mut mpsc::Receiver<u32>,
    request: u32,
    budget: Duration,
) -> Result<u32, RequestError> {
    let deadline = Instant::now() + budget;

    let remaining = deadline.saturating_duration_since(Instant::now());
    timeout(remaining, tx.send(request))
        .await
        .map_err(|_| RequestError::DeadlineExceeded)?
        .map_err(|_| RequestError::Disconnected)?;

    // WHY: the reply only gets whatever the send step left over.
    let remaining = deadline.saturating_duration_since(Instant::now());
    timeout(remaining, rx.recv())
        .await
        .map_err(|_| RequestError::DeadlineExceeded)?
        .ok_or(RequestError::Disconnected)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake provider: waits `accept_after` before taking the next request
    /// off the queue, then answers `request * 2` after `answer_after`.
    fn spawn_provider(
        mut requests: mpsc::Receiver<u32>,
        replies: mpsc::Sender<u32>,
        accept_after: Duration,
        answer_after: Duration,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(accept_after).await;
                let Some(request) = requests.recv().await else {
                    break;
                };
                tokio::time::sleep(answer_after).await;
                if replies.send(request * 2).await.is_err() {
                    break;
                }
            }
        });
    }

    #[tokio::test(start_paused = true)]
    async fn fits_in_budget() {
        let (req_tx, req_rx) = mpsc::channel(1);
        let (rep_tx, mut rep_rx) = mpsc::channel(1);
        spawn_provider(req_rx, rep_tx, Duration::ZERO, Duration::from_millis(600));
        let result = request_with_deadline(&req_tx, &mut rep_rx, 21, Duration::from_secs(1)).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test(start_paused = true)]
    async fn steps_share_one_budget() {
        let (req_tx, req_rx) = mpsc::channel(1);
        let (rep_tx, mut rep_rx) = mpsc::channel(1);
        req_tx.send(0).await.unwrap(); // queue is full: our send must wait
        spawn_provider(
            req_rx,
            rep_tx,
            Duration::from_millis(700),
            Duration::from_millis(400),
        );

        let start = Instant::now();
        let result = request_with_deadline(&req_tx, &mut rep_rx, 21, Duration::from_secs(1)).await;
        // The send waits 700ms for room, the first reply only arrives at
        // 1100ms. Each step alone is < 1s, together they blow the budget —
        // and we give up at exactly 1s, not at 1.7s.
        assert_eq!(result, Err(RequestError::DeadlineExceeded));
        assert_eq!(start.elapsed(), Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn disconnected_provider() {
        let (req_tx, req_rx) = mpsc::channel(1);
        let (_rep_tx, mut rep_rx) = mpsc::channel(1);
        drop(req_rx);
        let result = request_with_deadline(&req_tx, &mut rep_rx, 1, Duration::from_secs(1)).await;
        assert_eq!(result, Err(RequestError::Disconnected));
    }
}
