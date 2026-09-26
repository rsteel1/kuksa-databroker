// c13_e01_spawn_join_move.rs
//
// CONCEPT: `std::thread::spawn`, `move` closures, `JoinHandle::join`, and
//          scoped threads (`std::thread::scope`) that may borrow locals.
// GUIDE:   chapters/13-threads/guide.md  (section "Spawning threads: move and join")
// DATABROKER: databroker/src/main.rs — the Tokio runtime is built with
//          `worker_threads` = number of cores (line 418): real OS threads run
//          the broker's tasks in parallel.
// EXPECT:  compile error E0373 (closure may outlive the current function,
//          but it borrows `chunk`).
//
// `thread::spawn` requires a `'static` closure: the new thread might outlive
// the function that started it, so it may not borrow that function's locals.
// Either MOVE owned data into it, or use `thread::scope`, whose threads are
// guaranteed to finish before the scope ends (so borrowing is fine).
//
// YOUR TASK
//   1. Fix `sum_in_threads` (spawned threads must own their chunk).
//   2. Implement `max_in_scoped_threads` with `std::thread::scope`, borrowing
//      the slice — no cloning of the data.
//   Do not edit the tests.

use std::thread;

/// Splits `readings` into `parts` chunks, sums each chunk on its own thread
/// and returns the total.
fn sum_in_threads(readings: Vec<u64>, parts: usize) -> u64 {
    let chunk_size = readings.len().div_ceil(parts).max(1);
    let mut handles = Vec::new();
    for chunk in readings.chunks(chunk_size) {
        let chunk: Vec<u64> = chunk.to_vec();
        // WHY: `move` transfers ownership of `chunk` into the thread, which
        // satisfies spawn's `'static` requirement.
        handles.push(thread::spawn(move || chunk.iter().sum::<u64>()));
    }
    handles
        .into_iter()
        .map(|h| h.join().expect("worker thread panicked"))
        .sum()
}

/// Maximum of each half computed on two scoped threads; None if empty.
fn max_in_scoped_threads(readings: &[u64]) -> Option<u64> {
    let (left, right) = readings.split_at(readings.len() / 2);
    // WHY: scoped threads are joined before `scope` returns, so they may
    // borrow `left`/`right` without any copying.
    thread::scope(|s| {
        let l = s.spawn(|| left.iter().max().copied());
        let r = s.spawn(|| right.iter().max().copied());
        let l = l.join().expect("left thread panicked");
        let r = r.join().expect("right thread panicked");
        l.max(r)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_across_threads() {
        let data: Vec<u64> = (1..=100).collect();
        assert_eq!(sum_in_threads(data, 4), 5050);
        assert_eq!(sum_in_threads(vec![], 3), 0);
    }

    #[test]
    fn scoped_max() {
        assert_eq!(max_in_scoped_threads(&[3, 9, 2, 7, 1]), Some(9));
        assert_eq!(max_in_scoped_threads(&[5]), Some(5));
        assert_eq!(max_in_scoped_threads(&[]), None);
    }
}
