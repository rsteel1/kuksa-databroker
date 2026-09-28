// c13_e07_review_worker_bugs.rs
//
// CONCEPT: code review. The three functions below all compile and all
//          spawn real threads, but each has one planted bug typical of
//          thread-based code: a join result thrown away (so a worker panic
//          goes unnoticed), thread results reassembled at the wrong index,
//          and shared state read before the workers that update it have
//          been joined.
// GUIDE:   chapters/13-threads/guide.md  (section "Spawning threads: move and join")
// DATABROKER: the databroker never spawns an OS thread and drops the
//          handle — `start_housekeeping_task` (chapter 20's async version
//          of the same rule) always keeps and awaits its `JoinHandle`.
// EXPECT:  compiles; 3 of the tests below fail (one because a worker panic
//          is silently swallowed).
//
// THIS FILE HAS 3 BUGS. All three compile cleanly.
//
// YOUR TASK
//   Find and fix all 3 bugs. Do not change the tests or the signatures.
//   For each bug, add a one-line comment directly above your fix:
//   `// BUG 1: <what was wrong>`, `// BUG 2: ...`, `// BUG 3: ...`.

use std::sync::{Arc, Mutex};
use std::thread;

/// Runs `check` on every item, each on its own thread. Returns `true` only
/// if every check passed — a check that panics must count as a failure,
/// not be silently ignored.
fn all_pass<F>(items: Vec<i32>, check: F) -> bool
where
    F: Fn(i32) -> bool + Send + Sync + 'static + Copy,
{
    let mut handles = Vec::new();
    for item in items {
        handles.push(thread::spawn(move || {
            assert!(check(item), "check failed for {item}");
        }));
    }
    // BUG 1: `let _ = handle.join();` discards the `Result`, so a panicking
    // worker (an `Err`) was indistinguishable from a successful one, and
    // the function always returned `true` regardless. Track whether every
    // join actually succeeded.
    let mut all_ok = true;
    for handle in handles {
        if handle.join().is_err() {
            all_ok = false;
        }
    }
    all_ok
}

/// Computes `n * n` for every `n` in `nums`, each on its own thread, and
/// returns the results in the SAME order as `nums`.
fn parallel_squares(nums: Vec<i32>) -> Vec<i32> {
    let handles: Vec<_> = nums.iter().map(|&n| thread::spawn(move || n * n)).collect();

    let len = handles.len();
    let mut results = vec![0; len];
    for (i, handle) in handles.into_iter().enumerate() {
        // BUG 2: `(i + 1) % len` shifted every result one slot over from
        // where it belonged (and wrapped the last one back to index 0).
        results[i] = handle.join().unwrap();
    }
    results
}

/// Adds every amount in `amounts` to `counter`, each addition on its own
/// thread, and returns the FINAL value after every thread has finished.
fn total_after_increment(counter: Arc<Mutex<i32>>, amounts: Vec<i32>) -> i32 {
    let mut handles = Vec::new();
    for amount in amounts {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            *counter.lock().unwrap() += amount;
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }

    // BUG 3: the old code took its snapshot *before* spawning any worker
    // and returned that stale value — every thread's update had already
    // been joined by the time we return, so read the counter now, not
    // earlier.
    *counter.lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_pass_is_true_when_everything_passes() {
        assert!(all_pass(vec![1, 2, 3], |n| n > 0));
    }

    #[test]
    fn all_pass_is_false_when_a_check_panics() {
        assert!(!all_pass(vec![1, 2, -3, 4], |n| n > 0));
    }

    #[test]
    fn parallel_squares_preserves_order() {
        assert_eq!(parallel_squares(vec![1, 2, 3, 4]), vec![1, 4, 9, 16]);
    }

    #[test]
    fn total_after_increment_reflects_every_worker() {
        let counter = Arc::new(Mutex::new(10));
        let total = total_after_increment(Arc::clone(&counter), vec![1, 2, 3]);
        assert_eq!(total, 16);
        assert_eq!(*counter.lock().unwrap(), 16);
    }
}
