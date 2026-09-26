// c13_e03_std_mpsc_channel.rs
//
// CONCEPT: message passing with `std::sync::mpsc` — many producers, one
//          consumer; the channel closes when every `Sender` is dropped.
// GUIDE:   chapters/13-threads/guide.md  (section "Message passing with channels")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `open_provider_stream`
//          creates `mpsc::channel(10)` (line 929) and hands `Sender` clones to
//          code that answers the provider. That's tokio's async mpsc (chapter
//          14); the ownership rules are identical.
// EXPECT:  tests fail: `todo!()` panics.
//
// "Do not communicate by sharing memory; share memory by communicating."
// Instead of locking a map, producers SEND `(path, value)` messages and a
// single consumer owns the map. No locks at all.
//
// The receiver's `for msg in rx` loop ends only when ALL senders are gone.
// If you keep the original `tx` alive in the consumer's function, the loop
// never ends and the test hangs — drop it (or move it into the last
// producer) before draining the receiver.
//
// YOUR TASK
//   Implement `latest_values`: spawn one thread per producer batch that sends
//   each `(path, value)` in order; the calling thread receives everything and
//   keeps the last value it saw per path. Do not edit the tests.

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

type Update = (String, i64);

/// Returns (latest value per path, total number of messages received).
/// Each batch touches distinct paths, so "latest" is well-defined.
fn latest_values(batches: Vec<Vec<Update>>) -> (HashMap<String, i64>, usize) {
    let (tx, rx) = mpsc::channel::<Update>();
    for batch in batches {
        let tx = tx.clone();
        thread::spawn(move || {
            for update in batch {
                // WHY: send only fails if the receiver is gone; then there is
                // nobody left to tell, so stopping is the right reaction.
                if tx.send(update).is_err() {
                    break;
                }
            }
        });
    }
    // WHY: without this, our own `tx` keeps the channel open forever and the
    // loop below never ends.
    drop(tx);

    let mut latest = HashMap::new();
    let mut count = 0;
    for (path, value) in rx {
        latest.insert(path, value);
        count += 1;
    }
    (latest, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(path: &str, v: i64) -> Update {
        (path.to_string(), v)
    }

    /// Runs `latest_values` on a helper thread and fails (instead of hanging
    /// forever) if it doesn't finish, e.g. because a Sender was never dropped.
    fn run_with_timeout(batches: Vec<Vec<Update>>) -> (HashMap<String, i64>, usize) {
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = done_tx.send(latest_values(batches));
        });
        done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect(
            "latest_values never returned: is a Sender still alive, so `for msg in rx` never ends?",
        )
    }

    #[test]
    fn collects_latest_per_path() {
        let batches = vec![
            vec![
                u("Vehicle.Speed", 10),
                u("Vehicle.Speed", 20),
                u("Vehicle.Speed", 30),
            ],
            vec![u("Vehicle.Cabin.Temp", 19), u("Vehicle.Cabin.Temp", 21)],
            vec![],
        ];
        let (latest, count) = run_with_timeout(batches);
        assert_eq!(latest["Vehicle.Speed"], 30);
        assert_eq!(latest["Vehicle.Cabin.Temp"], 21);
        assert_eq!(count, 5);
    }

    #[test]
    fn no_producers_means_empty() {
        let (latest, count) = run_with_timeout(vec![]);
        assert!(latest.is_empty());
        assert_eq!(count, 0);
    }
}
