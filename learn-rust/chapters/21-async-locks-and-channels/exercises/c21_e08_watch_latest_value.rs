// c21_e08_watch_latest_value.rs
//
// CONCEPT: `tokio::sync::watch`: one slot holding the LATEST value, which
//          many receivers can read and await changes on.
// GUIDE:   chapters/21-async-locks-and-channels/guide.md  (section "watch: only the latest value matters")
// DATABROKER: a signal's current value is exactly "latest value wins" state.
//          The databroker uses `broadcast` for subscriptions because clients
//          may want every update (`buffer_size`); `watch` is the right tool
//          when only the newest value matters (config, health, the current
//          value of one signal).
// EXPECT:  tests fail: `todo!()` panics.
//
// broadcast = a queue of every message (receivers can lag and miss some).
// watch     = a single slot. Sending overwrites it. A receiver that was busy
//             sees only the newest value: intermediate values are coalesced,
//             and nothing ever "lags".
//
// YOUR TASK
//   Implement `latest`, `next_change` and `wait_until_above`.
//   Do not edit the tests.

// I AM NOT DONE

use tokio::sync::watch;

/// The value currently in the channel, without waiting and without marking
/// it as seen.
fn latest(rx: &watch::Receiver<u32>) -> u32 {
    todo!()
}

/// Waits for the value to change (relative to what this receiver last saw),
/// then returns the new value and marks it as seen.
/// Returns `None` once the sender has been dropped.
async fn next_change(rx: &mut watch::Receiver<u32>) -> Option<u32> {
    todo!()
}

/// Waits until the value is strictly greater than `limit` (it may already
/// be) and returns it. Returns `None` if the sender is dropped first.
async fn wait_until_above(rx: &mut watch::Receiver<u32>, limit: u32) -> Option<u32> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn latest_reads_the_slot() {
        let (tx, rx) = watch::channel(0);
        assert_eq!(latest(&rx), 0);
        tx.send(42).expect("receiver alive");
        assert_eq!(latest(&rx), 42);
    }

    #[tokio::test]
    async fn busy_receivers_only_see_the_newest_value() {
        let (tx, mut rx) = watch::channel(0);
        // Three sends before the receiver looks: the slot is overwritten.
        tx.send(10).expect("receiver alive");
        tx.send(20).expect("receiver alive");
        tx.send(30).expect("receiver alive");
        assert_eq!(next_change(&mut rx).await, Some(30));

        tx.send(40).expect("receiver alive");
        assert_eq!(next_change(&mut rx).await, Some(40));
    }

    #[tokio::test]
    async fn every_receiver_sees_the_value() {
        let (tx, mut a) = watch::channel(0);
        let mut b = a.clone();
        tx.send(7).expect("receivers alive");
        assert_eq!(next_change(&mut a).await, Some(7));
        assert_eq!(next_change(&mut b).await, Some(7));
    }

    #[tokio::test]
    async fn wait_until_above_returns_the_first_matching_value() {
        let (tx, mut rx) = watch::channel(50);
        let waiter = tokio::spawn(async move { wait_until_above(&mut rx, 100).await });
        tx.send(80).expect("receiver alive");
        tx.send(130).expect("receiver alive");
        let result = waiter.await.expect("task did not panic");
        // 80 never satisfied the predicate; the waiter returns once it sees
        // a value above 100 (130, or anything newer).
        assert!(matches!(result, Some(v) if v > 100));
    }

    #[tokio::test]
    async fn already_satisfied_returns_immediately() {
        let (_tx, mut rx) = watch::channel(120);
        assert_eq!(wait_until_above(&mut rx, 100).await, Some(120));
    }

    #[tokio::test]
    async fn dropped_sender_ends_the_wait() {
        let (tx, mut rx) = watch::channel(0);
        drop(tx);
        assert_eq!(next_change(&mut rx).await, None);
        assert_eq!(wait_until_above(&mut rx, 100).await, None);
    }
}
