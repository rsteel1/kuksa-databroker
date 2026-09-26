// c31_e03_graceful_shutdown.rs
//
// CONCEPT: graceful shutdown — one signal future, a broadcast trigger, tasks that `select!` on it.
// GUIDE:   chapters/31-building-a-service/guide.md  (section "Graceful shutdown")
// DATABROKER: databroker/src/main.rs — `shutdown_handler` (select! over SIGINT/SIGHUP/SIGTERM);
//             grpc/server.rs — `async fn shutdown(databroker, signal)`;
//             broker.rs — `DataBroker::shutdown`, `get_shutdown_trigger` (broadcast::Sender<()>);
//             grpc/kuksa_val_v2/val.rs — `open_provider_stream` loop: `_ = shutdown_trigger.recv() => break`
// EXPECT:  tests fail: `todo!()` panics
//
// When the databroker gets SIGTERM it must not just vanish: open provider
// streams should end cleanly, subscriptions get dropped, the socket is
// released. The shape:
//
//   signal future ──await──> broker.shutdown() ──send(())──> broadcast channel
//                                                              │ │ │
//                         every long-running task holds a Receiver and
//                         `select!`s on it next to its real work.
//
// A broadcast channel fits because EVERY task must see the one message.
// Tests pass a oneshot receiver as the "signal" so no real OS signal is needed.
//
// YOUR TASK
//   1. `Service::spawn_worker`: spawn a task that loops, `select!`ing between
//      `shutdown.recv()` (-> break) and `ticker.tick()` (-> increment `ticks`).
//      After the loop, increment `stopped`.
//   2. `Service::run_until`: await `signal`, send the shutdown trigger, then
//      await every worker's JoinHandle. Give the workers at most
//      `grace` in total (tokio::time::timeout); return Err(TimedOut) if they
//      don't all finish.
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
    ticks: Arc<AtomicUsize>,
    stopped: Arc<AtomicUsize>,
}

impl Service {
    fn new() -> Self {
        let (shutdown_trigger, _) = broadcast::channel(1);
        Service {
            shutdown_trigger,
            workers: Vec::new(),
            ticks: Arc::new(AtomicUsize::new(0)),
            stopped: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn spawn_worker(&mut self, period: Duration) {
        todo!("subscribe to the trigger BEFORE spawning, clone the Arcs, tokio::spawn(async move { loop { tokio::select! { ... } } })")
    }

    async fn run_until(
        self,
        signal: impl Future<Output = ()>,
        grace: Duration,
    ) -> Result<(), TimedOut> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[tokio::test(start_paused = true)]
    async fn workers_run_until_signal_then_stop() {
        let mut service = Service::new();
        service.spawn_worker(Duration::from_millis(100));
        service.spawn_worker(Duration::from_millis(100));
        let ticks = Arc::clone(&service.ticks);
        let stopped = Arc::clone(&service.stopped);

        let (tx, rx) = oneshot::channel::<()>();
        let signal = async {
            let _ = rx.await;
        };
        let runner = tokio::spawn(service.run_until(signal, Duration::from_secs(1)));

        tokio::time::sleep(Duration::from_millis(1050)).await;
        assert_eq!(stopped.load(Ordering::Relaxed), 0, "still running");
        let before = ticks.load(Ordering::Relaxed);
        assert!(before >= 20, "2 workers x ~11 ticks, got {before}");

        tx.send(()).unwrap();
        assert_eq!(runner.await.unwrap(), Ok(()));
        assert_eq!(
            stopped.load(Ordering::Relaxed),
            2,
            "every worker saw the trigger"
        );

        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(
            ticks.load(Ordering::Relaxed),
            before,
            "no ticks after shutdown"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn stuck_worker_hits_the_grace_period() {
        let mut service = Service::new();
        service.spawn_worker(Duration::from_millis(10));
        // A task that ignores the shutdown trigger entirely:
        service
            .workers
            .push(tokio::spawn(std::future::pending::<()>()));
        let result = service
            .run_until(std::future::ready(()), Duration::from_millis(500))
            .await;
        assert_eq!(result, Err(TimedOut));
    }

    #[tokio::test(start_paused = true)]
    async fn no_workers_is_fine() {
        let service = Service::new();
        let result = service
            .run_until(std::future::ready(()), Duration::from_millis(1))
            .await;
        assert_eq!(result, Ok(()));
    }
}
