// c31_e05_review_graceful_shutdown.rs
//
// CONCEPT: code review — this file COMPILES, but has 2 planted bugs typical
//          of hand-rolled graceful shutdown: a worker that subscribes to the
//          shutdown broadcast only AFTER it has been spawned (so it can miss
//          the message entirely), and a grace period that gets applied
//          separately to each worker instead of once to the whole batch.
// GUIDE:   chapters/31-building-a-service/guide.md  (section "Graceful shutdown")
// DATABROKER: databroker/src/broker.rs — every long-running task calls
//             `get_shutdown_trigger().subscribe()` and holds the `Receiver`
//             BEFORE it starts its main loop, never after; a `Sender::send`
//             on a `broadcast` channel only reaches receivers that already
//             exist at the moment it is called. databroker/src/grpc/server.rs:95-103
//             — `shutdown` sends the trigger once and then awaits
//             `databroker.shutdown()`, which joins every task under ONE
//             budget, not a fresh one per task.
// EXPECT:  tests fail: a worker whose task has not run yet when shutdown is
//          triggered keeps running for the rest of its work item instead of
//          stopping immediately, and two workers that never notice shutdown
//          together take about twice the configured grace period to give up
//          instead of about once.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`.
//   Do not edit the tests.

// I AM NOT DONE

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

#[derive(Debug, PartialEq)]
struct TimedOut;

struct Service {
    shutdown_trigger: broadcast::Sender<()>,
    workers: Vec<JoinHandle<()>>,
    stopped: Arc<AtomicUsize>,
}

impl Service {
    fn new() -> Self {
        let (shutdown_trigger, _) = broadcast::channel(1);
        Service {
            shutdown_trigger,
            workers: Vec::new(),
            stopped: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Spawns a task that runs for up to `work`, or stops early if it sees
    /// the shutdown trigger.
    fn spawn_worker(&mut self, work: Duration) {
        let stopped = Arc::clone(&self.stopped);
        let trigger = self.shutdown_trigger.clone();
        self.workers.push(tokio::spawn(async move {
            let mut shutdown = trigger.subscribe();
            tokio::select! {
                _ = shutdown.recv() => {}
                _ = tokio::time::sleep(work) => {}
            }
            stopped.fetch_add(1, Ordering::Relaxed);
        }));
    }

    /// Awaits `signal`, triggers shutdown, then waits for every worker to
    /// stop, bounded by `grace` for the whole batch.
    async fn run_until(
        self,
        signal: impl Future<Output = ()>,
        grace: Duration,
    ) -> Result<(), TimedOut> {
        signal.await;
        let _ = self.shutdown_trigger.send(());
        for worker in self.workers {
            if tokio::time::timeout(grace, worker).await.is_err() {
                return Err(TimedOut);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_worker_spawned_right_before_shutdown_still_stops() {
        let mut service = Service::new();
        service.spawn_worker(Duration::from_secs(10));
        // The worker's task has not been polled even once yet (we haven't
        // awaited anything since spawning it): if it only subscribes to the
        // trigger once it starts running, the shutdown message sent inside
        // `run_until` below is already gone by the time it gets there.
        let stopped = Arc::clone(&service.stopped);
        let result = service
            .run_until(std::future::ready(()), Duration::from_millis(500))
            .await;
        assert_eq!(
            result,
            Ok(()),
            "the worker should see the shutdown trigger and stop well within the grace period"
        );
        assert_eq!(stopped.load(Ordering::Relaxed), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn grace_bounds_the_whole_batch_not_each_worker() {
        let mut service = Service::new();
        // Two tasks that never finish on their own and never see the
        // shutdown trigger at all (they don't even hold a receiver) —
        // exactly the "stuck worker" case `grace` exists for.
        service
            .workers
            .push(tokio::spawn(std::future::pending::<()>()));
        service
            .workers
            .push(tokio::spawn(std::future::pending::<()>()));

        let start = tokio::time::Instant::now();
        let result = service
            .run_until(std::future::ready(()), Duration::from_millis(500))
            .await;
        assert_eq!(result, Err(TimedOut));
        assert!(
            start.elapsed() < Duration::from_millis(600),
            "grace is a budget for ALL workers together, not 500ms per worker; took {:?}",
            start.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn well_behaved_workers_stop_promptly() {
        let mut service = Service::new();
        service.spawn_worker(Duration::from_secs(10));
        service.spawn_worker(Duration::from_secs(10));
        let stopped = Arc::clone(&service.stopped);

        let result = service
            .run_until(std::future::ready(()), Duration::from_secs(1))
            .await;
        assert_eq!(result, Ok(()));
        assert_eq!(stopped.load(Ordering::Relaxed), 2);
    }
}
