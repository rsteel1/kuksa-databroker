// c21_e04_shutdown_trigger.rs
//
// CONCEPT: graceful shutdown with a `broadcast::Sender<()>` that every task
//          subscribes to.
// GUIDE:   chapters/21-async-locks-and-channels/guide.md  (section "The shutdown trigger pattern")
// DATABROKER: databroker/src/broker.rs — `DataBroker { shutdown_trigger:
//             broadcast::Sender<()> }`, `get_shutdown_trigger()` and
//             `shutdown()`; tasks in grpc/kuksa_val_v2/val.rs select! on
//             `shutdown_trigger.recv()`
// EXPECT:  tests fail: `todo!()` panics
//
// One `send(())` wakes every subscribed task. Important subtlety: a receiver
// only sees messages sent AFTER it subscribed — so subscribe before you spawn,
// never inside the spawned task.
//
// YOUR TASK
//   Implement `Broker::new`, `shutdown_trigger`, `shutdown` and
//   `spawn_worker`. A worker counts "ticks" (one every second, use
//   `tokio::time::sleep`) until shutdown, then returns its count.
//   Do not edit the tests.

use std::time::Duration;
use tokio::select;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

#[derive(Clone)]
struct Broker {
    shutdown_trigger: broadcast::Sender<()>,
}

impl Broker {
    fn new() -> Self {
        // WHY `_`: we don't need a receiver now; tasks call `shutdown_trigger()`.
        let (shutdown_trigger, _) = broadcast::channel(1);
        Broker { shutdown_trigger }
    }

    fn shutdown_trigger(&self) -> broadcast::Receiver<()> {
        self.shutdown_trigger.subscribe()
    }

    /// Signal every task. Must not fail when nobody is listening.
    fn shutdown(&self) {
        // WHY `let _ =`: `send` errors when there are zero receivers — fine.
        let _ = self.shutdown_trigger.send(());
    }
}

fn spawn_worker(broker: &Broker) -> JoinHandle<u32> {
    // WHY: subscribe *here*, before spawning, so a shutdown sent right after
    // spawn_worker returns can't be missed.
    let mut shutdown = broker.shutdown_trigger();
    tokio::spawn(async move {
        let mut ticks = 0;
        loop {
            select! {
                _ = tokio::time::sleep(Duration::from_secs(1)) => ticks += 1,
                _ = shutdown.recv() => break,
            }
        }
        ticks
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn all_workers_stop_on_shutdown() {
        let broker = Broker::new();
        let a = spawn_worker(&broker);
        let b = spawn_worker(&broker);
        tokio::time::sleep(Duration::from_millis(3500)).await;
        broker.shutdown();
        assert_eq!(a.await.unwrap(), 3);
        assert_eq!(b.await.unwrap(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn immediate_shutdown_is_not_missed() {
        let broker = Broker::new();
        let worker = spawn_worker(&broker);
        broker.shutdown(); // the worker task may not even have started yet
        assert_eq!(worker.await.unwrap(), 0);
    }

    #[tokio::test]
    async fn shutdown_without_listeners_is_fine() {
        Broker::new().shutdown();
    }
}
