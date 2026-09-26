// c27_e01_counting_allocator.rs
//
// CONCEPT: the global allocator (`GlobalAlloc`, `#[global_allocator]`), and
//          *measuring* instead of guessing: count heap allocations.
// GUIDE:   chapters/27-performance/guide.md  (section "Measure first: counting allocations")
// DATABROKER: databroker/src/main.rs:14-16 — with `--features jemalloc` the
//             databroker replaces the system allocator with jemalloc using
//             exactly this mechanism: `#[global_allocator] static ALLOC: ...`.
// EXPECT:  tests fail: todo!() panics
//
// Every `Box`, `Vec`, `String` and `HashMap` gets its memory from ONE global
// allocator. You can swap it for your own type that implements `GlobalAlloc`.
// We wrap the system allocator and count calls. Heap allocation is one of
// the most common hidden costs in Rust code, and the rest of this chapter
// uses this counter to *prove* optimisations work.
//
// The counter is thread-local: `cargo test` runs tests on several threads at
// once, and we only want to count the allocations of the code under test.
//
// YOUR TASK
//   1. Implement `bump` (add one to this thread's counter). It must never
//      panic: use `ALLOCS.try_with`, not `ALLOCS.with`, and ignore the
//      error. Inside an allocator a panic would try to allocate and abort.
//   2. Call `bump()` from `alloc`, `alloc_zeroed` and `realloc`, but NOT
//      from `dealloc` (freeing isn't allocating).
//   3. Implement `count_allocs`.
//   Do not edit the tests.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    // `const` initialisation: accessing it never allocates, so the
    // allocator can use it without recursing into itself.
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|count| count.set(count.get() + 1));
}

struct CountingAlloc;

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
        // SAFETY: `ptr` was allocated by `System` via the methods above with
        // this `layout`; the caller upholds `dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// Runs `f` and returns its result plus the number of (re)allocations it
/// performed on the current thread.
fn count_allocs<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let before = ALLOCS.with(Cell::get);
    let result = f();
    let after = ALLOCS.with(Cell::get);
    (result, after - before)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;

    #[test]
    fn nothing_allocates_nothing() {
        let (sum, n) = count_allocs(|| black_box(2) + black_box(3));
        assert_eq!(sum, 5);
        assert_eq!(n, 0);
    }

    #[test]
    fn empty_collections_do_not_allocate() {
        let (_, n) = count_allocs(|| {
            let v: Vec<u64> = black_box(Vec::new());
            let s = black_box(String::new());
            (v, s)
        });
        assert_eq!(n, 0, "Vec::new and String::new are free until first push");
    }

    #[test]
    fn a_box_is_one_allocation() {
        let (_, n) = count_allocs(|| black_box(Box::new(42_u64)));
        assert_eq!(n, 1);
    }

    #[test]
    fn with_capacity_avoids_regrowth() {
        let (_, grown) = count_allocs(|| {
            let mut v = Vec::new();
            for i in 0..1000_u64 {
                v.push(i);
            }
            black_box(v)
        });
        let (_, presized) = count_allocs(|| {
            let mut v = Vec::with_capacity(1000);
            for i in 0..1000_u64 {
                v.push(i);
            }
            black_box(v)
        });
        assert_eq!(presized, 1, "one up-front allocation");
        assert!(grown > 5, "growth reallocates repeatedly (saw {grown})");
    }

    #[test]
    fn freeing_is_not_counted() {
        let v = vec![1_u8; 64];
        let (_, n) = count_allocs(move || drop(black_box(v)));
        assert_eq!(n, 0);
    }
}
