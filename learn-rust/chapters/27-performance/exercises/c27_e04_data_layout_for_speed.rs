// c27_e04_data_layout_for_speed.rs
//
// CONCEPT: data layout decides speed: padding, struct-of-arrays vs
//          array-of-structs, and small inline storage that avoids the heap.
// GUIDE:   chapters/27-performance/guide.md  (section "Data layout: the cache is the real bottleneck")
// DATABROKER: databroker/src/broker.rs:113 `Entry` — one struct per signal
//             holding two `Datapoint`s, an optional target and `Metadata`
//             with several `String`s. Scanning every entry's value touches
//             all that memory. Compare `SignalsSoA` below. In the wild:
//             smallvec-1.16.2 src/lib.rs:833 `SmallVec` (the crate version of
//             `InlineVec`).
// EXPECT:  tests fail: todo!() panics
//
// A CPU loads memory in 64-byte cache lines. If a loop only needs `value`
// but every element also carries an id, a flag and padding, most of each
// line is wasted. Storing each field in its own `Vec` (struct of arrays)
// makes the loop read only what it uses.
//
// YOUR TASK
//   1. Implement `SignalsSoA::from_aos` and `SignalsSoA::sum_valid`.
//   2. Implement `InlineVec::push` and `InlineVec::as_slice`: up to N
//      elements live inline (no heap), the (N+1)th push moves everything to
//      a `Vec`.
//   Do not edit the tests.

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

/// Array-of-structs element. Note the field order: see the size test.
#[derive(Clone, Copy)]
struct SignalAoS {
    valid: bool,
    value: f64,
    id: u32,
}

/// Struct of arrays: one `Vec` per field.
#[derive(Default)]
struct SignalsSoA {
    ids: Vec<u32>,
    values: Vec<f64>,
    valid: Vec<bool>,
}

impl SignalsSoA {
    fn from_aos(signals: &[SignalAoS]) -> Self {
        let _ = signals;
        todo!()
    }

    /// Sum of `values[i]` where `valid[i]`. Zero allocations.
    fn sum_valid(&self) -> f64 {
        todo!("zip values with valid")
    }
}

/// A vector that stores up to `N` items inline and spills to the heap after.
#[derive(Debug)]
enum InlineVec<T: Copy + Default, const N: usize> {
    Inline { len: usize, items: [T; N] },
    Heap(Vec<T>),
}

impl<T: Copy + Default, const N: usize> InlineVec<T, N> {
    fn new() -> Self {
        InlineVec::Inline {
            len: 0,
            items: [T::default(); N],
        }
    }

    fn push(&mut self, item: T) {
        let _ = item;
        todo!()
    }

    fn as_slice(&self) -> &[T] {
        todo!()
    }

    fn is_inline(&self) -> bool {
        matches!(self, InlineVec::Inline { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    fn sample() -> Vec<SignalAoS> {
        (0..100)
            .map(|i| SignalAoS {
                valid: i % 2 == 0,
                value: f64::from(i),
                id: i,
            })
            .collect()
    }

    #[test]
    fn padding_is_real() {
        // bool(1) + u32(4) + f64(8) = 13 bytes of data, but alignment rounds
        // the struct up to 16. Rust already reorders fields to minimise
        // padding; `#[repr(C)]` would keep declaration order and use 24.
        assert_eq!(size_of::<SignalAoS>(), 16);
    }

    #[test]
    fn soa_sums_valid_values_without_allocating() {
        let soa = SignalsSoA::from_aos(&sample());
        assert_eq!(soa.ids.len(), 100);
        assert_eq!(soa.valid.iter().filter(|v| **v).count(), 50);
        let (sum, n) = count_allocs(|| soa.sum_valid());
        assert_eq!(sum, (0..100).filter(|i| i % 2 == 0).map(f64::from).sum());
        assert_eq!(n, 0);
    }

    #[test]
    fn small_stays_inline() {
        let (v, n) = count_allocs(|| {
            let mut v: InlineVec<u32, 4> = InlineVec::new();
            for i in 0..4 {
                v.push(i);
            }
            v
        });
        assert_eq!(n, 0, "four items fit inline: no heap");
        assert!(v.is_inline());
        assert_eq!(v.as_slice(), &[0, 1, 2, 3]);
    }

    #[test]
    fn spills_once() {
        let mut v: InlineVec<u32, 4> = InlineVec::new();
        for i in 0..4 {
            v.push(i);
        }
        let (_, n) = count_allocs(|| v.push(4));
        assert_eq!(n, 1, "one allocation when spilling");
        assert!(!v.is_inline());
        for i in 5..9 {
            v.push(i);
        }
        assert_eq!(v.as_slice(), &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
