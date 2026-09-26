// c25_e01_scoped_threads.rs
//
// CONCEPT: `std::thread::scope` in depth — mutable borrows of DISJOINT parts
//          of one buffer from several threads, returning values from scoped
//          threads, and two-phase parallel algorithms.
// GUIDE:   chapters/25-concurrency-in-depth/guide.md  (section "Scoped threads and disjoint borrows")
// DATABROKER: databroker/src/main.rs — the runtime's `worker_threads` (line 418)
//             are the databroker's parallelism; here you do the same thing by
//             hand for CPU work, without `Arc` or locks.
// EXPECT:  tests fail: `todo!()` panics
//
// Chapter 13 used `thread::scope` to *read* a shared slice. The borrow checker
// also lets several threads *write* at the same time, as long as each gets
// its own non-overlapping `&mut` piece: `chunks_mut` / `split_at_mut` prove
// disjointness, so there's no data race and no need for a Mutex.
//
// YOUR TASK
//   1. `scale_in_place`: multiply every element by `factor`, splitting the
//      slice into `threads` chunks, one scoped thread per chunk.
//   2. `normalize`: divide every element by the maximum absolute value, in
//      TWO parallel phases: (a) each thread returns the max of its chunk
//      (`ScopedJoinHandle::join`), (b) each thread scales its chunk. Leave an
//      all-zero slice unchanged.
//   3. `histogram`: count byte values in parallel, each thread filling its own
//      local `[usize; 256]`, merged at the end — no shared counters.
//   Do not edit the tests.

use std::thread;

/// Chunk length so that `len` items are split into at most `threads` chunks.
fn chunk_len(len: usize, threads: usize) -> usize {
    len.div_ceil(threads.max(1)).max(1)
}

fn scale_in_place(values: &mut [f64], factor: f64, threads: usize) {
    let size = chunk_len(values.len(), threads);
    thread::scope(|s| {
        for chunk in values.chunks_mut(size) {
            // WHY: each `chunk` is a distinct `&mut [f64]`; the scope guarantees
            // every thread ends before `values` is usable again.
            s.spawn(move || {
                for v in chunk {
                    *v *= factor;
                }
            });
        }
    });
}

fn normalize(values: &mut [f64], threads: usize) {
    let size = chunk_len(values.len(), threads);

    // Phase 1: parallel max. Scoped threads can RETURN values via `join`.
    let max = thread::scope(|s| {
        let handles: Vec<_> = values
            .chunks(size)
            .map(|chunk| s.spawn(move || chunk.iter().fold(0.0_f64, |m, v| m.max(v.abs()))))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("max thread panicked"))
            .fold(0.0_f64, f64::max)
    });

    if max == 0.0 {
        return;
    }
    // Phase 2: parallel scale (the shared borrow from phase 1 has ended).
    scale_in_place(values, 1.0 / max, threads);
}

fn histogram(data: &[u8], threads: usize) -> [usize; 256] {
    let size = chunk_len(data.len(), threads);
    thread::scope(|s| {
        let handles: Vec<_> = data
            .chunks(size)
            .map(|chunk| {
                s.spawn(move || {
                    let mut local = [0usize; 256];
                    for &b in chunk {
                        local[b as usize] += 1;
                    }
                    local
                })
            })
            .collect();
        // WHY: merging private results once is far cheaper than every thread
        // hammering shared atomics or a Mutex for each byte.
        let mut total = [0usize; 256];
        for h in handles {
            let local = h.join().expect("histogram thread panicked");
            for (t, l) in total.iter_mut().zip(local) {
                *t += l;
            }
        }
        total
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_touches_every_element_once() {
        let mut v: Vec<f64> = (0..1001).map(f64::from).collect();
        scale_in_place(&mut v, 2.0, 4);
        let expected: Vec<f64> = (0..1001).map(|i| f64::from(i) * 2.0).collect();
        assert_eq!(v, expected);
    }

    #[test]
    fn scale_handles_more_threads_than_items_and_empty() {
        let mut v = vec![1.0, 2.0];
        scale_in_place(&mut v, 10.0, 16);
        assert_eq!(v, vec![10.0, 20.0]);
        let mut empty: Vec<f64> = Vec::new();
        scale_in_place(&mut empty, 3.0, 4);
        assert!(empty.is_empty());
    }

    #[test]
    fn normalize_divides_by_max_abs() {
        let mut v = vec![2.0, -8.0, 4.0, 1.0, 0.0];
        normalize(&mut v, 3);
        assert_eq!(v, vec![0.25, -1.0, 0.5, 0.125, 0.0]);
    }

    #[test]
    fn normalize_leaves_all_zero_alone() {
        let mut v = vec![0.0; 10];
        normalize(&mut v, 4);
        assert_eq!(v, vec![0.0; 10]);
    }

    #[test]
    fn histogram_matches_sequential_count() {
        let data: Vec<u8> = (0..100_000u32).map(|i| (i * 7 % 256) as u8).collect();
        let mut expected = [0usize; 256];
        for &b in &data {
            expected[b as usize] += 1;
        }
        assert_eq!(histogram(&data, 8), expected);
        assert_eq!(histogram(&[], 8), [0usize; 256]);
    }
}
