// c27_e03_iterators_are_zero_cost.rs
//
// CONCEPT: iterator chains compile to the same loops you'd write by hand,
//          often better, because they skip bounds checks; `windows`,
//          `chunks_exact` and exact-size `collect`.
// GUIDE:   chapters/27-performance/guide.md  (section "Zero-cost iterators and bounds checks")
// DATABROKER: databroker/src/broker.rs:2254-2262 — `providers.into_iter().map(...)`
//             feeding `join_all`, and the `iter().map().collect()` pipelines
//             throughout conversions.rs. None of them cost more than a loop.
// EXPECT:  tests fail: todo!() panics
//
// Performance isn't timed in unit tests (too noisy). These tests check
// correctness and allocation counts, and the guide shows how to check the
// generated assembly for bounds checks with `cargo asm`.
//
// YOUR TASK
//   Implement the functions with iterator adaptors: no index loops
//   (`for i in 0..len`) and no `values[i]`. Do not edit the tests.

// I AM NOT DONE

#![allow(unsafe_code)] // only for the provided counting allocator below

// ---------------------------------------------------------------------------
// Provided: the counting allocator you built in c27_e01. It counts every
// allocation and reallocation made *by the current thread*, so tests running
// in parallel on other threads don't disturb the numbers.
// ---------------------------------------------------------------------------
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        // `const` initialisation: accessing it never allocates, so the
        // allocator can use it without recursing into itself.
        static ALLOCS: Cell<usize> = const { Cell::new(0) };
    }

    fn bump() {
        // `try_with` instead of `with`: never panic inside the allocator,
        // even while a thread is being torn down.
        let _ = ALLOCS.try_with(|count| count.set(count.get() + 1));
    }

    pub struct CountingAlloc;

    // SAFETY: every method forwards to `System` with the caller's arguments
    // unchanged, so all of `GlobalAlloc`'s contracts are upheld by `System`.
    // The only extra work touches a const-initialised thread-local `Cell`,
    // which neither allocates nor unwinds.
    unsafe impl GlobalAlloc for CountingAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `alloc`'s contract; passed through.
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `alloc_zeroed`'s contract; passed through.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `realloc`'s contract; passed through.
            unsafe { System.realloc(ptr, layout, new_size) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: `ptr` was allocated by `System` via the methods above
            // with this `layout`; the caller upholds `dealloc`'s contract.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static GLOBAL: CountingAlloc = CountingAlloc;

    /// Runs `f` and returns its result plus the number of (re)allocations
    /// it performed on the current thread.
    pub fn count_allocs<R>(f: impl FnOnce() -> R) -> (R, usize) {
        let before = ALLOCS.with(Cell::get);
        let result = f();
        let after = ALLOCS.with(Cell::get);
        (result, after - before)
    }
}
use counting::count_allocs;

/// Sum of squares of all readings.
fn sum_of_squares(values: &[i64]) -> i64 {
    let _ = values;
    todo!()
}

/// Average of every window of `window` consecutive readings.
/// Budget: one allocation (the output). `window == 0` -> empty result.
fn moving_average(values: &[f64], window: usize) -> Vec<f64> {
    let _ = (values, window);
    todo!()
}

/// XOR checksum of a byte stream read as little-endian u32 words.
/// A trailing partial word is zero-padded. Budget: zero allocations.
fn checksum(bytes: &[u8]) -> u32 {
    let _ = bytes;
    todo!("hint: chunks_exact(4) and its remainder()")
}

/// Ids of the readings above `limit`, in order. Budget: one allocation.
fn ids_above(readings: &[(u32, f64)], limit: f64) -> Vec<u32> {
    let _ = (readings, limit);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squares() {
        assert_eq!(sum_of_squares(&[1, -2, 3]), 14);
        assert_eq!(sum_of_squares(&[]), 0);
    }

    #[test]
    fn moving_average_allocates_once() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0];
        let (avg, n) = count_allocs(|| moving_average(&values, 2));
        assert_eq!(avg, [1.5, 2.5, 3.5, 4.5]);
        assert_eq!(n, 1);
        assert!(moving_average(&values, 6).is_empty());
        assert!(moving_average(&values, 0).is_empty());
    }

    #[test]
    fn checksum_handles_tail_without_allocating() {
        let bytes = [0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0xFF];
        let (sum, n) = count_allocs(|| checksum(&bytes));
        assert_eq!(sum, 0x01 ^ 0x03 ^ 0xFF);
        assert_eq!(n, 0);
        assert_eq!(checksum(&[]), 0);
    }

    #[test]
    fn filter_then_collect_once() {
        let readings: Vec<(u32, f64)> = (0..1000).map(|i| (i, f64::from(i % 10))).collect();
        let (ids, n) = count_allocs(|| ids_above(&readings, 7.5));
        assert_eq!(ids.len(), 200);
        assert_eq!(&ids[..4], &[8, 9, 18, 19]);
        assert_eq!(
            n, 1,
            "ids_above allocated {n} times: filter() hides the size"
        );
    }
}
