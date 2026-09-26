// c13_e02_arc_mutex_counter.rs
//
// CONCEPT: sharing mutable state between `'static` threads with
//          `Arc<Mutex<T>>`: Arc for shared ownership, Mutex for exclusive access.
// GUIDE:   chapters/13-threads/guide.md  (section "Shared state: Arc<Mutex<T>>")
// DATABROKER: databroker/src/broker.rs — `DataBroker { database:
//          Arc<RwLock<Database>>, subscriptions: Arc<RwLock<Subscriptions>>, ... }`
//          (line 179); `start_housekeeping_task` clones the Arc before moving
//          it into the spawned task (line 2364).
// EXPECT:  tests fail: `todo!()` panics.
//
// Pattern: clone the `Arc` (cheap) *before* the `move` closure, move the
// clone in, lock inside the thread, keep the guard's scope small.
//
// YOUR TASK
//   Implement `count_updates`: spawn one `thread::spawn` thread per batch;
//   each thread increments the shared per-path counter for each path in its
//   batch. Return the final counts. Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

fn count_updates(batches: Vec<Vec<String>>) -> HashMap<String, u32> {
    let counts: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn counts_across_threads() {
        let batches = vec![
            batch(&["Vehicle.Speed", "Vehicle.Horn"]),
            batch(&["Vehicle.Speed"]),
            batch(&["Vehicle.Speed", "Vehicle.Cabin.Temp"]),
        ];
        let counts = count_updates(batches);
        assert_eq!(counts["Vehicle.Speed"], 3);
        assert_eq!(counts["Vehicle.Horn"], 1);
        assert_eq!(counts["Vehicle.Cabin.Temp"], 1);
        assert_eq!(counts.len(), 3);
    }

    #[test]
    fn many_threads_no_lost_updates() {
        let batches = (0..32).map(|_| batch(&["X"; 100])).collect();
        assert_eq!(count_updates(batches)["X"], 3200);
    }
}
