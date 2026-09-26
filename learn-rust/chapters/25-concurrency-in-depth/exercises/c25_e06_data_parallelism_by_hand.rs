// c25_e06_data_parallelism_by_hand.rs
//
// CONCEPT: data parallelism — static vs dynamic scheduling, an atomic
//          "next chunk" counter (the core idea behind rayon's work stealing),
//          and reassembling results in input order.
// GUIDE:   chapters/25-concurrency-in-depth/guide.md  (section "Data parallelism by hand")
// DATABROKER: databroker/src/broker.rs — `get_values_broker` (line 2254) runs
//             provider calls concurrently with `join_all` and then restores the
//             request order with an `IndexMap` (line 2323). Same shape: fan out,
//             then put results back in order.
// EXPECT:  tests fail: `todo!()` panics
//
// Splitting work into `threads` equal pieces (static scheduling, as in e01)
// is bad when items take very different amounts of time: one thread gets all
// the slow items while the others idle. Dynamic scheduling fixes it: workers
// repeatedly grab the next small chunk from a shared atomic counter until
// none are left.
//
// YOUR TASK
//   Implement `par_map`: apply `f` to every item using `threads` scoped
//   threads that pull chunks of `chunk_size` items via an `AtomicUsize`
//   counter (`fetch_add`). Return the results in input order. No `unsafe`,
//   no Mutex around the output: let each thread return `(chunk_index,
//   Vec<R>)` pairs and assemble at the end.
//   Do not edit the tests.

// I AM NOT DONE

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

fn par_map<T, R, F>(items: &[T], threads: usize, chunk_size: usize, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;
    use std::time::Duration;

    #[test]
    fn results_are_in_input_order() {
        let items: Vec<u64> = (0..10_000).collect();
        let out = par_map(&items, 4, 37, |x| x * x);
        let expected: Vec<u64> = items.iter().map(|x| x * x).collect();
        assert_eq!(out, expected);
    }

    #[test]
    fn edge_cases() {
        let empty: Vec<u32> = Vec::new();
        assert!(par_map(&empty, 4, 8, |x| *x).is_empty());
        assert_eq!(par_map(&[1, 2, 3], 16, 100, |x| x + 1), vec![2, 3, 4]);
        assert_eq!(par_map(&[1, 2, 3], 1, 1, |x| x * 10), vec![10, 20, 30]);
    }

    #[test]
    fn every_item_is_processed_exactly_once() {
        let items: Vec<usize> = (0..5_000).collect();
        let seen = Mutex::new(Vec::new());
        par_map(&items, 8, 13, |x| seen.lock().unwrap().push(*x));
        let mut seen = seen.into_inner().unwrap();
        seen.sort_unstable();
        assert_eq!(seen, items);
    }

    #[test]
    fn uses_at_most_the_requested_number_of_threads() {
        let items: Vec<u32> = (0..1_000).collect();
        let ids = Mutex::new(HashSet::new());
        par_map(&items, 3, 10, |_| {
            ids.lock().unwrap().insert(thread::current().id());
        });
        let n = ids.lock().unwrap().len();
        assert!((1..=3).contains(&n), "used {n} threads");
    }

    #[test]
    fn slow_items_are_spread_dynamically() {
        // The first 4 items are slow. With static scheduling one thread would
        // get all of them; dynamic chunk-grabbing spreads them out.
        let items: Vec<u32> = (0..64).collect();
        let workers_on_slow = Mutex::new(HashSet::new());
        par_map(&items, 4, 1, |x| {
            if *x < 4 {
                workers_on_slow
                    .lock()
                    .unwrap()
                    .insert(thread::current().id());
                thread::sleep(Duration::from_millis(20));
            }
        });
        assert!(
            workers_on_slow.lock().unwrap().len() >= 2,
            "slow items should not all land on one worker"
        );
    }
}
