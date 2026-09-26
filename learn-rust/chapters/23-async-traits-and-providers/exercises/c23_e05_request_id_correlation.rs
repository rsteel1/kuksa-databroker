// c23_e05_request_id_correlation.rs
//
// CONCEPT: request/response over a long-lived bidirectional stream:
//          request ids from an `AtomicU32`, skipping stale responses,
//          serialising exchanges with a `tokio::sync::Mutex`, one deadline.
// GUIDE:   chapters/23-async-traits-and-providers/guide.md  (section "Talking to a provider over its stream")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs —
//             `impl SignalProvider for Provider` → `get_signals_values_from_provider`
//             (`next_request_id.fetch_add(1, Ordering::Relaxed)`,
//             "Skipping provider response with unexpected request_id",
//             "Only accept entries for the signals we actually requested",
//             "Reject empty response from provider to trigger fallback")
// EXPECT:  tests fail: `todo!()` panics
//
// A provider is connected through ONE stream in each direction. The broker
// sends `GetProviderValueRequest { request_id, signal_ids }` and later reads
// `GetProviderValueResponse { request_id, entries }`. Responses for an older,
// timed-out request may still arrive — the request id tells them apart.
//
// YOUR TASK
//   Implement `ProviderConnection::get`:
//     - the whole call (lock, send, wait for the answer) must finish within
//       `DEADLINE`, otherwise `Err(())`
//     - hold the `inbox` mutex for the whole exchange (one request at a time)
//     - take a fresh request id from `next_request_id`
//     - skip responses whose `request_id` doesn't match
//     - drop entries for ids that weren't requested
//     - an empty result, a closed channel, or a lagged inbox -> `Err(())`
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::{timeout, Instant};

const DEADLINE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
struct Request {
    request_id: u32,
    signal_ids: Vec<i32>,
}

#[derive(Debug, Clone)]
struct Response {
    request_id: u32,
    entries: HashMap<i32, f64>,
}

struct ProviderConnection {
    outbox: mpsc::Sender<Request>,
    inbox: Mutex<broadcast::Receiver<Response>>,
    next_request_id: AtomicU32,
}

impl ProviderConnection {
    fn new(outbox: mpsc::Sender<Request>, inbox: broadcast::Receiver<Response>) -> Self {
        ProviderConnection {
            outbox,
            inbox: Mutex::new(inbox),
            next_request_id: AtomicU32::new(1),
        }
    }

    async fn get(&self, signal_ids: Vec<i32>) -> Result<HashMap<i32, f64>, ()> {
        todo!()
    }
}

// ---- a fake provider on the other end of the "stream" ---------------------

/// What the fake provider does with each request it receives.
#[derive(Clone, Copy)]
enum Behaviour {
    /// Answer every requested id with `id * 10`.
    Echo,
    /// First send a stale response (wrong request id), then the real one,
    /// plus one entry nobody asked for.
    StaleThenEcho,
    /// Never answer.
    Silent,
    /// Answer with no entries.
    Empty,
}

fn spawn_fake_provider(behaviour: Behaviour) -> ProviderConnection {
    let (request_tx, mut request_rx) = mpsc::channel::<Request>(4);
    let (response_tx, response_rx) = broadcast::channel::<Response>(8);
    tokio::spawn(async move {
        while let Some(request) = request_rx.recv().await {
            let echo: HashMap<i32, f64> = request
                .signal_ids
                .iter()
                .map(|&id| (id, f64::from(id) * 10.0))
                .collect();
            let reply = |request_id, entries| Response {
                request_id,
                entries,
            };
            match behaviour {
                Behaviour::Echo => {
                    let _ = response_tx.send(reply(request.request_id, echo));
                }
                Behaviour::StaleThenEcho => {
                    let _ = response_tx
                        .send(reply(request.request_id + 100, HashMap::from([(1, -1.0)])));
                    let mut entries = echo;
                    entries.insert(999, 0.0);
                    let _ = response_tx.send(reply(request.request_id, entries));
                }
                Behaviour::Silent => {}
                Behaviour::Empty => {
                    let _ = response_tx.send(reply(request.request_id, HashMap::new()));
                }
            }
        }
    });
    ProviderConnection::new(request_tx, response_rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test(start_paused = true)]
    async fn simple_request() {
        let connection = spawn_fake_provider(Behaviour::Echo);
        assert_eq!(
            connection.get(vec![1, 2]).await,
            Ok(HashMap::from([(1, 10.0), (2, 20.0)]))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn skips_stale_responses_and_unrequested_entries() {
        let connection = spawn_fake_provider(Behaviour::StaleThenEcho);
        assert_eq!(
            connection.get(vec![1]).await,
            Ok(HashMap::from([(1, 10.0)]))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn silent_provider_hits_the_deadline() {
        let connection = spawn_fake_provider(Behaviour::Silent);
        let start = Instant::now();
        assert_eq!(connection.get(vec![1]).await, Err(()));
        assert_eq!(start.elapsed(), DEADLINE);
    }

    #[tokio::test(start_paused = true)]
    async fn empty_response_means_fallback() {
        let connection = spawn_fake_provider(Behaviour::Empty);
        assert_eq!(connection.get(vec![1]).await, Err(()));
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_callers_get_their_own_answers() {
        let connection = Arc::new(spawn_fake_provider(Behaviour::Echo));
        let a = {
            let connection = connection.clone();
            tokio::spawn(async move { connection.get(vec![1]).await })
        };
        let b = {
            let connection = connection.clone();
            tokio::spawn(async move { connection.get(vec![2]).await })
        };
        assert_eq!(a.await.unwrap(), Ok(HashMap::from([(1, 10.0)])));
        assert_eq!(b.await.unwrap(), Ok(HashMap::from([(2, 20.0)])));
        // Two requests were made, so the next id is 3.
        assert_eq!(connection.next_request_id.load(Ordering::Relaxed), 3);
    }
}
