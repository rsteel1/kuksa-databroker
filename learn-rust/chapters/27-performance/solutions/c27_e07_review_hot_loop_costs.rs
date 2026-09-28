// c27_e07_review_hot_loop_costs.rs
//
// CONCEPT: code review — this file COMPILES and returns the right answers,
//          but has 2 planted bugs typical of chapter 27 (a dedup that scans
//          the whole output for every input, making it accidentally O(n^2),
//          and an update loop that clones the entire buffer on every
//          iteration instead of writing through it). Both are caught by
//          *counting* operations, never by timing.
// GUIDE:   chapters/27-performance/guide.md  (sections "Measure first:
//          counting allocations" and "Zero-cost iterators and bounds checks")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs:206 —
//             `let requested_ids: HashSet<i32> = signals_ids.iter().map(|id| id.id()).collect();`
//             turns repeated membership checks into O(1) lookups instead of
//             an O(n) scan per check. databroker/src/broker.rs:2324 pre-sizes
//             its `IndexMap` up front instead of growing it one entry at a
//             time, for the same reason `apply_overrides` below should
//             write into its buffer instead of re-cloning it.
// EXPECT:  tests fail: the allocation/comparison-budget assertions (the
//          results are already correct).
//
// Both functions below already return the right answer. `dedup_signal_ids`
// makes far more comparisons than it needs to, and `apply_overrides`
// allocates far more than it needs to. On a hot path (every subscription
// update, every batch of overrides) that turns into CPU burned on work that
// produces nothing new.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`. Keep both
//   functions' signatures and their results unchanged.
//   Do not edit the tests, and do not edit the `counting` module.

#![allow(unsafe_code)] // only for the provided counting allocator below

use std::cell::Cell;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Provided: counts every comparison made between two `CountedId`s, so a test
// can tell "linear number of comparisons" apart from "quadratic" without
// ever measuring wall-clock time.
// ---------------------------------------------------------------------------
thread_local! {
    static COMPARISONS: Cell<usize> = const { Cell::new(0) };
}

fn comparisons() -> usize {
    COMPARISONS.with(Cell::get)
}

fn reset_comparisons() {
    COMPARISONS.with(|c| c.set(0));
}

#[derive(Debug, Clone, Copy, Eq)]
struct CountedId(u64);

impl PartialEq for CountedId {
    fn eq(&self, other: &Self) -> bool {
        COMPARISONS.with(|c| c.set(c.get() + 1));
        self.0 == other.0
    }
}

/// Returns `ids` with duplicates removed, keeping first-seen order.
fn dedup_signal_ids(ids: &[CountedId]) -> Vec<CountedId> {
    // BUG 1: `result.contains(id)` rescans the whole (growing) output for
    // every input id, an O(n) check inside an O(n) loop — O(n^2) overall.
    // Track which raw ids have been seen in a `HashSet<u64>` instead: that
    // membership check is O(1) and never touches `CountedId::eq` at all.
    let mut seen = HashSet::with_capacity(ids.len());
    let mut result = Vec::with_capacity(ids.len());
    for id in ids {
        if seen.insert(id.0) {
            result.push(*id);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Provided: the counting allocator from c27_e01, counting every allocation
// and reallocation made *by the current thread*.
// ---------------------------------------------------------------------------
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static ALLOCS: Cell<usize> = const { Cell::new(0) };
    }

    fn bump() {
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

/// Applies each `(index, value)` override to a copy of `base`, in order.
fn apply_overrides(base: &[String], overrides: &[(usize, String)]) -> Vec<String> {
    let mut result = base.to_vec();
    for (idx, value) in overrides {
        // BUG 2: this cloned the entire current `result` into `fresh` just
        // to overwrite one element, on every iteration — O(base.len()) work
        // per override instead of O(1). Write through the buffer directly.
        result[*idx] = value.clone();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_keeps_first_seen_order() {
        let ids: Vec<CountedId> = [3, 1, 3, 2, 1, 4].into_iter().map(CountedId).collect();
        let deduped = dedup_signal_ids(&ids);
        assert_eq!(
            deduped
                .into_iter()
                .map(|CountedId(n)| n)
                .collect::<Vec<_>>(),
            vec![3, 1, 2, 4]
        );
    }

    #[test]
    fn dedup_signal_ids_is_linear_in_the_input_size() {
        reset_comparisons();
        let unique: Vec<CountedId> = (0..300).map(CountedId).collect();
        let with_dupes: Vec<CountedId> = unique
            .iter()
            .copied()
            .chain(unique.iter().copied())
            .collect();

        let deduped = dedup_signal_ids(&with_dupes);
        assert_eq!(deduped, unique, "dedup_signal_ids changed the result");

        let n = with_dupes.len();
        let seen = comparisons();
        assert!(
            seen <= 4 * n,
            "dedup_signal_ids made {seen} comparisons for {n} ids — that's \
             quadratic (n^2/2 would be {}), not linear",
            n * n / 2
        );
    }

    #[test]
    fn apply_overrides_produces_the_right_values() {
        let base: Vec<String> = (0..5).map(|i| format!("v{i}")).collect();
        let overrides = vec![(1, "one".to_string()), (3, "three".to_string())];
        let result = apply_overrides(&base, &overrides);
        assert_eq!(result, vec!["v0", "one", "v2", "three", "v4"]);
    }

    #[test]
    fn apply_overrides_does_not_reclone_the_whole_buffer_per_update() {
        let base: Vec<String> = (0..50).map(|i| format!("v{i}")).collect();
        let overrides: Vec<(usize, String)> = (0..20)
            .map(|i| (i % base.len(), format!("new{i}")))
            .collect();

        let mut expected = base.clone();
        for (idx, value) in &overrides {
            expected[*idx] = value.clone();
        }

        let (result, allocs) = count_allocs(|| apply_overrides(&base, &overrides));
        assert_eq!(result, expected);

        let budget = base.len() + overrides.len() + 5;
        assert!(
            allocs <= budget,
            "apply_overrides made {allocs} allocations for {} overrides over a \
             {}-element buffer (budget {budget}) — looks like it's cloning \
             the whole buffer on every iteration instead of writing through it",
            overrides.len(),
            base.len()
        );
    }
}
