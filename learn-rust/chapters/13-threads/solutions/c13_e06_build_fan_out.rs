// c13_e06_build_fan_out.rs
//
// CONCEPT: a small thread-pool fan-out built on `std::thread::scope`: split
//          the work into chunks, run each chunk on its own thread borrowing
//          from the caller (no `Arc` needed — the scope guarantees every
//          thread joins before it returns), then reassemble the results in
//          the original order.
// GUIDE:   chapters/13-threads/guide.md  (section "Spawning threads: move and join")
// DATABROKER: databroker/src/broker.rs's `get_values_broker` fans a request
//          out to every provider and gathers the results back; this
//          exercise is the CPU-bound, synchronous shape of the same idea
//          (the real databroker uses async tasks instead of OS threads —
//          chapter 20 covers that version).
// EXPECT:  compiles; every test fails (`todo!()` panics) until you fill in
//          the two `todo!()` bodies below.
//
// YOUR TASK
//   Build this from the spec below. Nothing here is a bug to fix — you are
//   writing new code. Do not edit the tests or the signatures.
//
//   1. `fan_out_map`: split `items` into `num_workers` contiguous chunks
//      (as even as `chunks(len.div_ceil(num_workers))` gives you), spawn
//      one scoped thread per chunk that maps `f` over every element of its
//      chunk into a `Vec<R>`, join every thread, and flatten the per-chunk
//      results back into one `Vec<R>` in original order. `num_workers` may
//      be larger than `items.len()`, or `items` may be empty — handle both
//      without panicking.
//   2. `count_exceeding`: using `fan_out_map`, count how many `readings`
//      are strictly greater than `threshold`. (Map each chunk to its own
//      count, then sum those counts — don't write a second hand-rolled
//      thread-spawning loop.)

use std::thread;

/// Applies `f` to every element of `items`, spread across `num_workers`
/// threads, preserving input order.
fn fan_out_map<T, R, F>(items: &[T], num_workers: usize, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    if items.is_empty() {
        return Vec::new();
    }
    let num_workers = num_workers.max(1);
    let chunk_len = items.len().div_ceil(num_workers);

    thread::scope(|scope| {
        // WHY: `scope.spawn` borrows `chunk` and `f` for the scope's
        // lifetime instead of requiring `Arc`/`move`+clone — the scope
        // itself guarantees every handle is joined before it returns.
        let handles: Vec<_> = items
            .chunks(chunk_len)
            .map(|chunk| scope.spawn(|| chunk.iter().map(&f).collect::<Vec<R>>()))
            .collect();

        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("worker thread panicked"))
            .collect()
    })
}

/// How many `readings` are strictly greater than `threshold`, computed
/// across `num_workers` threads via `fan_out_map`.
fn count_exceeding(readings: &[f64], threshold: f64, num_workers: usize) -> usize {
    fan_out_map(readings, num_workers, |&r| usize::from(r > threshold))
        .into_iter()
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_out_map_preserves_order() {
        let items = vec![1, 2, 3, 4, 5, 6, 7];
        let doubled = fan_out_map(&items, 3, |x| x * 2);
        assert_eq!(doubled, vec![2, 4, 6, 8, 10, 12, 14]);
    }

    #[test]
    fn fan_out_map_handles_empty_input() {
        let items: Vec<i32> = Vec::new();
        assert_eq!(fan_out_map(&items, 4, |x| x * 2), Vec::<i32>::new());
    }

    #[test]
    fn fan_out_map_handles_more_workers_than_items() {
        let items = vec![10, 20];
        assert_eq!(fan_out_map(&items, 8, |x| x + 1), vec![11, 21]);
    }

    #[test]
    fn fan_out_map_works_with_a_single_worker() {
        let items = vec![1, 2, 3];
        assert_eq!(fan_out_map(&items, 1, |x| x * x), vec![1, 4, 9]);
    }

    #[test]
    fn count_exceeding_matches_a_naive_count() {
        let readings: Vec<f64> = (0..100).map(|n| n as f64).collect();
        let naive = readings.iter().filter(|&&r| r > 42.0).count();
        assert_eq!(count_exceeding(&readings, 42.0, 5), naive);
        assert_eq!(count_exceeding(&readings, 42.0, 1), naive);
        assert_eq!(count_exceeding(&readings, 42.0, 33), naive);
    }
}
