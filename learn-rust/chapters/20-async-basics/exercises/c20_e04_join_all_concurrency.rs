// c20_e04_join_all_concurrency.rs
//
// CONCEPT: concurrency without threads — awaiting many futures at once
//          with `futures::future::join_all`.
// GUIDE:   chapters/20-async-basics/guide.md  (section "Concurrency: join and join_all")
// DATABROKER: databroker/src/broker.rs — `get_values_broker` builds one future
//             per provider and awaits them with `futures::future::join_all`
//             (commit 748a914 "Query providers concurrently")
// EXPECT:  tests fail: `todo!()` panics
//
// `a.await; b.await;` runs a THEN b. To overlap the waiting, create all the
// futures first and await them together. `join_all` returns the outputs in
// the same order as the input futures, no matter which finished first.
//
// Before commit 748a914 the databroker asked providers one after another, so
// three slow providers tripled the latency of a GetValues call.
//
// YOUR TASK
//   Implement `query_concurrently`. The sequential version is given for
//   comparison. Do not use `tokio::spawn` — `join_all` is enough.
//   Do not edit the tests.

// I AM NOT DONE

use futures::future::join_all;
use std::time::Duration;
use tokio::time::sleep;

/// A provider that takes `delay_ms` to answer with its own delay.
async fn query_provider(delay_ms: u64) -> u64 {
    sleep(Duration::from_millis(delay_ms)).await;
    delay_ms
}

async fn query_sequentially(delays_ms: &[u64]) -> Vec<u64> {
    let mut answers = Vec::new();
    for &delay in delays_ms {
        answers.push(query_provider(delay).await);
    }
    answers
}

async fn query_concurrently(delays_ms: &[u64]) -> Vec<u64> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;

    #[tokio::test(start_paused = true)]
    async fn sequential_adds_up_latencies() {
        let start = Instant::now();
        assert_eq!(
            query_sequentially(&[300, 100, 200]).await,
            vec![300, 100, 200]
        );
        assert_eq!(start.elapsed(), Duration::from_millis(600));
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_takes_the_slowest_latency() {
        let start = Instant::now();
        assert_eq!(
            query_concurrently(&[300, 100, 200]).await,
            vec![300, 100, 200]
        );
        assert_eq!(start.elapsed(), Duration::from_millis(300));
    }

    #[tokio::test(start_paused = true)]
    async fn no_providers() {
        assert!(query_concurrently(&[]).await.is_empty());
    }
}
