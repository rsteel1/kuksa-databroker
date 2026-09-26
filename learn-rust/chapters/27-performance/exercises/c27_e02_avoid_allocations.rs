// c27_e02_avoid_allocations.rs
//
// CONCEPT: the four everyday allocation killers: pre-size (`with_capacity`),
//          borrow instead of own (`&str`), allocate only when needed (`Cow`),
//          and reuse buffers (`clear()` + `write!`).
// GUIDE:   chapters/27-performance/guide.md  (section "Fewer allocations: the everyday toolkit")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs:403 —
//             `HashMap::with_capacity(size)` before inserting one entry per
//             requested path, and broker.rs:2324 `IndexMap::with_capacity(...)`.
//             Both know the final size up front, so they allocate once.
// EXPECT:  tests fail: the allocation-budget assertions (results are correct already)
//
// Every function below already returns the right answer. It just allocates
// far more than it needs to. On a hot path (every published value, every
// subscriber notification) that turns into allocator contention and cache
// misses.
//
// YOUR TASK
//   Rewrite the four function bodies so they meet the budgets in the tests.
//   Keep the signatures. Do not edit the tests.

// I AM NOT DONE

#![allow(unsafe_code)] // only for the provided counting allocator below

use std::borrow::Cow;

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

/// Joins segments with '.'. Budget: exactly one allocation.
fn join_path(segments: &[&str]) -> String {
    let owned: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
    let mut out = String::new();
    for (i, segment) in owned.iter().enumerate() {
        if i > 0 {
            out = out + ".";
        }
        out = out + segment;
    }
    out
}

/// Converts '/'-separated paths to VSS dots. Budget: zero allocations when
/// the input has no '/', one otherwise.
fn normalize(path: &str) -> Cow<'_, str> {
    Cow::Owned(path.replace('/', "."))
}

/// Renders `path=value` lines into `buf`, replacing its previous content.
/// Budget: zero allocations when `buf` already has enough capacity.
fn render_updates(buf: &mut String, updates: &[(&str, f64)]) {
    *buf = updates
        .iter()
        .map(|(path, value)| format!("{path}={value}\n"))
        .collect();
}

/// Counts paths starting with `prefix`, ignoring ASCII case.
/// Budget: zero allocations.
fn count_matching(paths: &[String], prefix: &str) -> usize {
    paths
        .iter()
        .filter(|path| path.to_lowercase().starts_with(&prefix.to_lowercase()))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_is_one_allocation() {
        let (joined, n) = count_allocs(|| join_path(&["Vehicle", "Cabin", "Door", "IsOpen"]));
        assert_eq!(joined, "Vehicle.Cabin.Door.IsOpen");
        assert_eq!(n, 1, "join_path allocated {n} times");
        assert_eq!(join_path(&[]), "");
        assert_eq!(join_path(&["Vehicle"]), "Vehicle");
    }

    #[test]
    fn normalize_borrows_when_it_can() {
        let (p, n) = count_allocs(|| normalize("Vehicle.Speed"));
        assert!(matches!(p, Cow::Borrowed("Vehicle.Speed")));
        assert_eq!(n, 0, "no '/' means no allocation (saw {n})");

        let (p, n) = count_allocs(|| normalize("Vehicle/Cabin/Temp"));
        assert_eq!(p, "Vehicle.Cabin.Temp");
        assert_eq!(n, 1);
    }

    #[test]
    fn render_reuses_the_buffer() {
        let updates = [("Vehicle.Speed", 88.5), ("Vehicle.Cabin.Temp", 21.0)];
        let mut buf = String::new();
        render_updates(&mut buf, &updates);
        assert_eq!(buf, "Vehicle.Speed=88.5\nVehicle.Cabin.Temp=21\n");

        let (_, n) = count_allocs(|| render_updates(&mut buf, &updates));
        assert_eq!(buf, "Vehicle.Speed=88.5\nVehicle.Cabin.Temp=21\n");
        assert_eq!(n, 0, "second render must reuse the buffer (saw {n})");
    }

    #[test]
    fn matching_does_not_allocate() {
        let paths: Vec<String> = ["Vehicle.Speed", "vehicle.cabin", "Body.Lights", "Ve"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (count, n) = count_allocs(|| count_matching(&paths, "VEHICLE."));
        assert_eq!(count, 2);
        assert_eq!(n, 0, "count_matching allocated {n} times");
    }
}
