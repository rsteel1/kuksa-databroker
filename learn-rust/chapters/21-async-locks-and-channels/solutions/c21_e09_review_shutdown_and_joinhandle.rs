// c21_e09_review_shutdown_and_joinhandle.rs
//
// CONCEPT: code review — this file COMPILES, but has 2 planted bugs typical
//          of chapter 21 (a `tokio::spawn`'s `JoinHandle` dropped instead of
//          kept, so shutdown can't tell when the worker actually finished,
//          and a shutdown branch that stops the loop without draining
//          whatever was already queued).
// GUIDE:   chapters/21-async-locks-and-channels/guide.md  (section "The shutdown trigger pattern")
// DATABROKER: databroker/src/broker.rs — `DataBroker::shutdown()` signals
//             every subscriber and the caller awaits the tasks' join
//             handles; dropping a `JoinHandle` detaches the task, so
//             `shutdown()` would return before the worker is actually done.
// EXPECT:  tests fail: after `shutdown()` returns, some items sent to the
//          worker were never processed.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`. You may
//   add a field to `Worker`.
//   Do not edit the tests, and do not change any function's signature.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

struct Worker {
    shutdown_tx: watch::Sender<bool>,
    // BUG 1: the task's `JoinHandle` was thrown away; without it nothing
    // can ever wait for the worker to finish draining and exit.
    handle: JoinHandle<()>,
}

/// Spawns a worker that increments `processed` for every item it receives,
/// until told to shut down.
fn spawn_worker(mut rx: mpsc::Receiver<u32>, processed: Arc<AtomicU32>) -> Worker {
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                item = rx.recv() => {
                    match item {
                        Some(_) => { processed.fetch_add(1, Ordering::SeqCst); }
                        None => break,
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        // BUG 2: breaking here without draining lost every
                        // item that was already queued but not yet pulled
                        // out of `rx` by the `recv()` branch above (`select!`
                        // can observe the shutdown signal before it happens
                        // to poll `recv()` again). Drain whatever is left
                        // before exiting.
                        while let Ok(_item) = rx.try_recv() {
                            processed.fetch_add(1, Ordering::SeqCst);
                        }
                        break;
                    }
                }
            }
        }
    });
    Worker { shutdown_tx, handle }
}

/// Signals the worker to stop and waits for it to actually finish.
async fn shutdown(worker: Worker) {
    let _ = worker.shutdown_tx.send(true);
    let _ = worker.handle.await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_waits_for_every_queued_item_to_be_processed() {
        let (tx, rx) = mpsc::channel(16);
        let processed = Arc::new(AtomicU32::new(0));
        let worker = spawn_worker(rx, processed.clone());

        for i in 0..5u32 {
            tx.try_send(i).expect("channel has room");
        }
        drop(tx);

        shutdown(worker).await;

        assert_eq!(processed.load(Ordering::SeqCst), 5);
    }
}
