// c24_e08_checkpoint_part3.rs
//
// CONCEPT: CHECKPOINT for Part III (async) — tasks, channels, shutdown,
//          timeouts and deadlines, concurrency limits, ordering of results.
// GUIDE:   chapters/24-async-internals/guide.md  (section "Checkpoint")
// DATABROKER: databroker/src/broker.rs — `get_values_broker` (line 2200) fans
//             requests out to providers concurrently under a time budget and
//             reassembles results in request order; `open_provider_stream`
//             tasks stop on the broadcast shutdown trigger.
// EXPECT:  tests fail: `todo!()` panics
//
// Minimal scaffolding on purpose: you design the internals.
//
// YOUR TASK
//   Implement `fan_out`. It runs `handler` for every request and returns one
//   `Outcome` per request, IN REQUEST ORDER:
//     * at most `config.workers` handler calls may be in flight at once;
//     * there is one overall deadline, `config.budget` after the call starts.
//       A call still running at the deadline is cancelled -> `TimedOut`.
//       A request not yet started at the deadline -> `NotStarted`;
//     * once `shutdown` fires, no NEW request is started (-> `NotStarted`);
//       calls already running may finish (still bounded by the deadline);
//     * `Ok(s)` -> `Done(s)`, `Err(e)` -> `Failed(e)`.
//   Requests are started in order (request i+1 never starts before request i).
//   Do not edit the tests.

// I AM NOT DONE

use std::future::Future;
use std::time::Duration;
use tokio::sync::broadcast;

#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    Done(String),
    Failed(String),
    TimedOut,
    NotStarted,
}

struct FanOutConfig {
    workers: usize,
    budget: Duration,
}

async fn fan_out<H, Fut>(
    requests: Vec<String>,
    config: FanOutConfig,
    shutdown: broadcast::Receiver<()>,
    handler: H,
) -> Vec<Outcome>
where
    H: Fn(String) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String, String>> + Send + 'static,
{
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Request format: "<millis>:<ok|err>:<label>"
    async fn fake_handler(req: String) -> Result<String, String> {
        let mut parts = req.splitn(3, ':');
        let ms: u64 = parts.next().unwrap().parse().unwrap();
        let ok = parts.next().unwrap() == "ok";
        let label = parts.next().unwrap().to_string();
        tokio::time::sleep(Duration::from_millis(ms)).await;
        if ok {
            Ok(label)
        } else {
            Err(format!("bad request: {label}"))
        }
    }

    fn reqs(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn no_shutdown() -> (broadcast::Sender<()>, broadcast::Receiver<()>) {
        broadcast::channel(1)
    }

    #[tokio::test(start_paused = true)]
    async fn results_come_back_in_request_order() {
        let (_tx, rx) = no_shutdown();
        let out = fan_out(
            reqs(&["30:ok:a", "10:ok:b", "20:err:c", "5:ok:d"]),
            FanOutConfig {
                workers: 4,
                budget: Duration::from_secs(1),
            },
            rx,
            fake_handler,
        )
        .await;
        assert_eq!(
            out,
            vec![
                Outcome::Done("a".into()),
                Outcome::Done("b".into()),
                Outcome::Failed("bad request: c".into()),
                Outcome::Done("d".into()),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn concurrency_is_capped_at_workers() {
        let (_tx, rx) = no_shutdown();
        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let (inf, max) = (in_flight.clone(), max_seen.clone());
        let handler = move |req: String| {
            let (inf, max) = (inf.clone(), max.clone());
            async move {
                let now = inf.fetch_add(1, Ordering::SeqCst) + 1;
                max.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(10)).await;
                inf.fetch_sub(1, Ordering::SeqCst);
                Ok(req)
            }
        };
        let requests: Vec<String> = (0..10).map(|i| i.to_string()).collect();
        let out = fan_out(
            requests,
            FanOutConfig {
                workers: 3,
                budget: Duration::from_secs(1),
            },
            rx,
            handler,
        )
        .await;
        assert_eq!(out.len(), 10);
        assert!(out.iter().all(|o| matches!(o, Outcome::Done(_))));
        assert_eq!(max_seen.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn the_budget_times_out_running_calls_and_skips_the_rest() {
        let (_tx, rx) = no_shutdown();
        let start = tokio::time::Instant::now();
        let out = fan_out(
            reqs(&["50:ok:fast", "500:ok:slow", "50:ok:never"]),
            FanOutConfig {
                workers: 1,
                budget: Duration::from_millis(200),
            },
            rx,
            fake_handler,
        )
        .await;
        assert_eq!(
            out,
            vec![
                Outcome::Done("fast".into()),
                Outcome::TimedOut,
                Outcome::NotStarted
            ]
        );
        assert!(
            start.elapsed() <= Duration::from_millis(210),
            "fan_out must return at the deadline, not wait for the slow call"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_stops_new_work_but_lets_running_calls_finish() {
        let (tx, rx) = broadcast::channel(1);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            tx.send(()).unwrap();
        });
        // One worker, 30 ms each: #0 runs 0..30, #1 runs 30..60 (shutdown at 50
        // happens while it runs), #2 would start at 60 -> not started.
        let out = fan_out(
            reqs(&["30:ok:zero", "30:ok:one", "30:ok:two", "30:ok:three"]),
            FanOutConfig {
                workers: 1,
                budget: Duration::from_secs(10),
            },
            rx,
            fake_handler,
        )
        .await;
        assert_eq!(
            out,
            vec![
                Outcome::Done("zero".into()),
                Outcome::Done("one".into()),
                Outcome::NotStarted,
                Outcome::NotStarted
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn empty_input_returns_immediately() {
        let (_tx, rx) = no_shutdown();
        let out = fan_out(
            Vec::new(),
            FanOutConfig {
                workers: 2,
                budget: Duration::from_millis(100),
            },
            rx,
            fake_handler,
        )
        .await;
        assert!(out.is_empty());
    }
}
