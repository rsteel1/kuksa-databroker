// c21_e03_broadcast_and_lag.rs
//
// CONCEPT: `tokio::sync::broadcast` — every receiver sees every message,
//          unless it falls more than `capacity` behind: then it LAGS.
// GUIDE:   chapters/21-async-locks-and-channels/guide.md  (section "broadcast: fan-out and lag")
// DATABROKER: databroker/src/broker.rs — each `ChangeSubscription` owns a
//             `broadcast::Sender<Option<EntryUpdates>>`; `subscribe` sizes the
//             channel as `buffer_size + 1` and logs "Slow subscriber ... lagged,
//             missed {} signal updates"
// EXPECT:  tests fail: `todo!()` panics
//
// Broadcast never blocks the sender. If a receiver is too slow, the oldest
// messages are overwritten and that receiver's next `recv()` returns
// `Err(RecvError::Lagged(n))` — "you missed n messages" — after which it
// continues with the oldest message still in the buffer. For a databroker
// that's the right trade-off: one slow subscriber must never slow down
// sensor updates for everyone else.
//
// YOUR TASK
//   Implement `receive_all`: collect values until the channel is closed,
//   counting how many were missed through lag.
//   Do not edit the tests.

// I AM NOT DONE

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

/// Returns (values received, number of values missed due to lag).
async fn receive_all(mut rx: broadcast::Receiver<i32>) -> (Vec<i32>, u64) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fast_receivers_see_everything() {
        let (tx, rx1) = broadcast::channel(16);
        let rx2 = tx.subscribe();
        for v in 1..=3 {
            tx.send(v).unwrap();
        }
        drop(tx);
        assert_eq!(receive_all(rx1).await, (vec![1, 2, 3], 0));
        assert_eq!(receive_all(rx2).await, (vec![1, 2, 3], 0));
    }

    #[tokio::test]
    async fn slow_receiver_lags() {
        let (tx, rx) = broadcast::channel(2);
        for v in 1..=5 {
            tx.send(v).unwrap(); // never blocks, even though nobody reads
        }
        drop(tx);
        // Only the newest `capacity` values survive.
        assert_eq!(receive_all(rx).await, (vec![4, 5], 3));
    }

    #[tokio::test]
    async fn late_subscribers_only_see_new_messages() {
        let (tx, _keep_alive) = broadcast::channel(8);
        tx.send(1).unwrap();
        let late = tx.subscribe();
        tx.send(2).unwrap();
        drop(tx);
        assert_eq!(receive_all(late).await, (vec![2], 0));
    }

    #[test]
    fn send_fails_without_receivers() {
        let (tx, rx) = broadcast::channel::<i32>(1);
        drop(rx);
        assert!(tx.send(1).is_err());
        assert_eq!(tx.receiver_count(), 0); // housekeeping uses exactly this
    }
}
