// c24_e06_cancellation_and_drop.rs
//
// CONCEPT: cancellation = dropping a future. `Drop` runs mid-await;
//          cancellation-safe state machines for `select!` loops; task panics
//          and aborts surfacing as `JoinError`.
// GUIDE:   chapters/24-async-internals/guide.md  (section "Cancellation, Drop and task failure")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — the provider loop
//             `select! { message = stream.message() => ..., _ = shutdown_trigger.recv() => break }`
//             (line 938) is only correct because `Streaming::message()` is
//             cancellation safe: dropping it never loses a half-read message.
// EXPECT:  tests fail: `todo!()` panics
//
// When `select!` picks one branch, `timeout` expires, or a `JoinHandle` is
// aborted, the losing future is DROPPED at whatever `.await` it was parked
// on. No more code of it runs — except `Drop` impls of the values it holds.
// So:
//   * cleanup belongs in `Drop` (RAII guards), not in code after an `.await`;
//   * any state that must survive a cancelled attempt must live OUTSIDE the
//     future (in a struct the loop owns), not in its local variables.
//
// YOUR TASK
//   1. `InFlight` guard: increments the gauge on creation and decrements it in
//      `Drop`, so the count is right even when the work is cancelled.
//   2. `FrameReader::next_frame`: a frame is a header (the body length) followed
//      by a body, received as two separate messages. Make it cancellation
//      safe: if the future is dropped after the header arrived, the next call
//      must continue with that header instead of losing it.
//   3. `supervise`: await a `JoinHandle` and classify the outcome as
//      `Ok(value)`, `Err(TaskFailure::Panicked(msg))` or
//      `Err(TaskFailure::Cancelled)`.
//   Do not edit the tests.

// I AM NOT DONE

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

// ----------------------------------------------------------------- 1. guard

struct InFlight {
    gauge: Arc<AtomicUsize>,
}

impl InFlight {
    fn start(gauge: &Arc<AtomicUsize>) -> Self {
        todo!()
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        todo!()
    }
}

async fn tracked_work(gauge: Arc<AtomicUsize>, work: std::time::Duration) -> &'static str {
    let _guard = InFlight::start(&gauge);
    tokio::time::sleep(work).await;
    "finished"
}

// ---------------------------------------------------- 2. cancellation safety

/// Messages arrive as `Header(len)` followed by `Body(bytes)`.
#[derive(Debug)]
enum Msg {
    Header(usize),
    Body(Vec<u8>),
}

struct FrameReader {
    rx: mpsc::Receiver<Msg>,
    /// A header we already received whose body hasn't arrived yet.
    pending_len: Option<usize>,
}

impl FrameReader {
    fn new(rx: mpsc::Receiver<Msg>) -> Self {
        FrameReader {
            rx,
            pending_len: None,
        }
    }

    /// Returns the next complete frame body, or `None` when the channel closes.
    /// Panics if the protocol is violated (body without header, or a body whose
    /// length doesn't match its header).
    async fn next_frame(&mut self) -> Option<Vec<u8>> {
        todo!()
    }
}

// ------------------------------------------------------ 3. task supervision

#[derive(Debug, PartialEq)]
enum TaskFailure {
    Panicked(String),
    Cancelled,
}

async fn supervise<T>(handle: JoinHandle<T>) -> Result<T, TaskFailure> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn guard_decrements_when_work_completes() {
        let gauge = Arc::new(AtomicUsize::new(0));
        let out = tracked_work(gauge.clone(), Duration::from_millis(10)).await;
        assert_eq!(out, "finished");
        assert_eq!(gauge.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn guard_decrements_when_work_is_cancelled_by_timeout() {
        let gauge = Arc::new(AtomicUsize::new(0));
        let work = tracked_work(gauge.clone(), Duration::from_secs(60));
        let res = tokio::time::timeout(Duration::from_millis(10), work).await;
        assert!(res.is_err(), "timed out");
        assert_eq!(gauge.load(Ordering::SeqCst), 0, "Drop ran on cancellation");
    }

    #[tokio::test(start_paused = true)]
    async fn guard_counts_concurrent_work_then_returns_to_zero() {
        let gauge = Arc::new(AtomicUsize::new(0));
        let tasks: Vec<_> = (0..3)
            .map(|_| tokio::spawn(tracked_work(gauge.clone(), Duration::from_millis(50))))
            .collect();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(gauge.load(Ordering::SeqCst), 3);
        tasks[0].abort();
        for t in tasks {
            let _ = t.await;
        }
        assert_eq!(gauge.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn frames_survive_select_cancellation() {
        let (tx, rx) = mpsc::channel(8);
        let mut reader = FrameReader::new(rx);
        let mut ticks = tokio::time::interval(Duration::from_millis(10));
        ticks.tick().await; // first tick is immediate

        tokio::spawn(async move {
            tx.send(Msg::Header(3)).await.unwrap();
            // The body arrives only after several ticks have fired, so the
            // `next_frame` future is dropped (by select!) while it holds the header.
            tokio::time::sleep(Duration::from_millis(35)).await;
            tx.send(Msg::Body(vec![1, 2, 3])).await.unwrap();
            tx.send(Msg::Header(1)).await.unwrap();
            tx.send(Msg::Body(vec![9])).await.unwrap();
        });

        let mut frames = Vec::new();
        let mut tick_count = 0;
        loop {
            tokio::select! {
                frame = reader.next_frame() => match frame {
                    Some(f) => frames.push(f),
                    None => break,
                },
                _ = ticks.tick() => tick_count += 1,
            }
        }
        assert!(
            tick_count >= 3,
            "select! cancelled next_frame several times"
        );
        assert_eq!(frames, vec![vec![1, 2, 3], vec![9]]);
    }

    #[tokio::test]
    async fn supervise_reports_success_panic_and_cancellation() {
        assert_eq!(supervise(tokio::spawn(async { 7 })).await, Ok(7));

        let panicking = tokio::spawn(async {
            let sensor = "Vehicle.Speed";
            if !sensor.is_empty() {
                panic!("provider for {sensor} crashed");
            }
            0
        });
        assert_eq!(
            supervise(panicking).await,
            Err(TaskFailure::Panicked(
                "provider for Vehicle.Speed crashed".to_string()
            ))
        );

        let static_msg = tokio::spawn(async { panic!("boom") });
        assert_eq!(
            supervise(static_msg).await,
            Err(TaskFailure::Panicked("boom".to_string()))
        );

        let forever = tokio::spawn(std::future::pending::<()>());
        forever.abort();
        assert_eq!(supervise(forever).await, Err(TaskFailure::Cancelled));
    }
}
