// c20_e06_blocking_and_send.rs
//
// CONCEPT: (1) futures handed to `tokio::spawn` must be `Send` — holding a
//          `std::sync::MutexGuard` across `.await` breaks that;
//          (2) CPU-heavy work belongs in `spawn_blocking`, not in async code.
// GUIDE:   chapters/20-async-basics/guide.md  (section "Send futures and blocking work")
// DATABROKER: databroker/src/broker.rs — the broker uses `tokio::sync::RwLock`
//             for `database`/`subscriptions` precisely because its guards
//             are held across `.await` (see `update_entries`)
// EXPECT:  compile error: "future cannot be sent between threads safely"
//          (`std::sync::MutexGuard<'_, u32>` is not `Send`)
//
// Everything a future holds *at an .await point* is stored inside the future.
// Tokio may resume it on a different worker thread, so it must be `Send`.
// A `std::sync::MutexGuard` is not `Send` (the OS lock must be released by
// the thread that took it).
//
// YOUR TASK
//   1. Fix `record_reading` so the std mutex guard is dropped BEFORE the
//      `.await`. Keep using `std::sync::Mutex` (it is the right tool for a
//      tiny critical section with no await inside).
//   2. Implement `checksum_in_background` with `tokio::task::spawn_blocking`.
//   Do not edit the tests.

use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Adds `reading` to the shared total, then pretends to notify someone.
async fn record_reading(total: Arc<Mutex<u32>>, reading: u32) -> u32 {
    // WHY: the guard lives only inside this block, so it is gone before the
    // `.await` below and the future stays `Send`.
    let new_total = {
        let mut guard = total.lock().expect("mutex poisoned");
        *guard += reading;
        *guard
    };
    tokio::time::sleep(Duration::from_millis(1)).await; // "notify subscribers"
    new_total
}

/// Deliberately CPU-bound: would stall a Tokio worker thread if run inline.
fn expensive_checksum(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Run `expensive_checksum` on Tokio's blocking thread pool.
async fn checksum_in_background(data: Vec<u8>) -> u64 {
    // WHY: the closure must own `data` ('static), hence `move`.
    tokio::task::spawn_blocking(move || expensive_checksum(&data))
        .await
        .expect("checksum task panicked")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn spawned_recording_works() {
        let total = Arc::new(Mutex::new(0));
        // `tokio::spawn` is what demands a `Send` future.
        let a = tokio::spawn(record_reading(total.clone(), 5));
        let b = tokio::spawn(record_reading(total.clone(), 7));
        let (a, b) = (a.await.unwrap(), b.await.unwrap());
        assert_eq!(*total.lock().unwrap(), 12);
        assert!(a == 12 || b == 12);
    }

    #[tokio::test]
    async fn checksum_matches_inline_version() {
        let data = b"Vehicle.Speed".to_vec();
        let expected = expensive_checksum(&data);
        assert_eq!(checksum_in_background(data).await, expected);
    }
}
