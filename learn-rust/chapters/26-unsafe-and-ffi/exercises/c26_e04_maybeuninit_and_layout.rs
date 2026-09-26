// c26_e04_maybeuninit_and_layout.rs
//
// CONCEPT: `MaybeUninit<T>` for memory that is not initialised yet; panic
//          safety (a guard whose `Drop` cleans up a half-built value).
// GUIDE:   chapters/26-unsafe-and-ffi/guide.md  (section "Uninitialised memory: MaybeUninit")
// DATABROKER: none. In the wild: tokio's mpsc channel stores its slots as
//             `[UnsafeCell<MaybeUninit<T>>; BLOCK_CAP]` (tokio-1.53.1
//             src/sync/mpsc/block.rs:47) and initialises them one by one.
// EXPECT:  tests fail: todo!() panics
//
// `std::array::from_fn` already does this safely. Writing it yourself shows
// what such a function has to guarantee:
//   * never read or drop a slot that hasn't been written;
//   * if the element constructor panics halfway, drop the elements that WERE
//     written (otherwise they leak) and nothing else (dropping an
//     uninitialised slot is UB);
//   * only at the end, reinterpret `[MaybeUninit<T>; N]` as `[T; N]`.
//
// YOUR TASK
//   Implement `build_array`. A `Guard` struct is provided: it remembers how
//   many slots are initialised, and its `Drop` must drop exactly those.
//   Fill in `Guard::drop` as well. Use `std::mem::forget(guard)` once every
//   slot is written, so the guard doesn't drop elements you're returning.
//   Do not edit the tests.

// I AM NOT DONE

#![allow(unsafe_code)]

use std::mem::MaybeUninit;

struct Guard<'a, T, const N: usize> {
    slots: &'a mut [MaybeUninit<T>; N],
    initialised: usize,
}

impl<T, const N: usize> Drop for Guard<'_, T, N> {
    fn drop(&mut self) {
        todo!("drop exactly the first `initialised` slots")
    }
}

/// Builds `[T; N]` by calling `make(i)` for i in 0..N, in order.
fn build_array<T, const N: usize>(mut make: impl FnMut(usize) -> T) -> [T; N] {
    let mut slots: [MaybeUninit<T>; N] = [const { MaybeUninit::uninit() }; N];
    let _ = (&mut slots, &mut make);
    todo!("write each slot, track progress in a Guard, then reinterpret")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// Counts how many times it is dropped.
    struct Tracked<'a> {
        id: usize,
        drops: &'a Cell<usize>,
    }
    impl Drop for Tracked<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn builds_like_from_fn() {
        let a: [u64; 5] = build_array(|i| (i * i) as u64);
        assert_eq!(a, std::array::from_fn::<u64, 5, _>(|i| (i * i) as u64));
        let paths: [String; 3] = build_array(|i| format!("Vehicle.Row{i}"));
        assert_eq!(paths[2], "Vehicle.Row2");
    }

    #[test]
    fn zero_length_is_fine() {
        let a: [String; 0] = build_array(|_| unreachable!());
        assert!(a.is_empty());
    }

    #[test]
    fn every_element_dropped_exactly_once() {
        let drops = Cell::new(0);
        {
            let a: [Tracked; 4] = build_array(|id| Tracked { id, drops: &drops });
            assert_eq!(a[3].id, 3);
            assert_eq!(drops.get(), 0, "nothing dropped while building");
        }
        assert_eq!(drops.get(), 4);
    }

    #[test]
    fn panic_midway_drops_only_the_built_prefix() {
        let drops = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _: [Tracked; 5] = build_array(|id| {
                if id == 3 {
                    panic!("sensor 3 failed");
                }
                Tracked { id, drops: &drops }
            });
        }));
        assert!(result.is_err());
        assert_eq!(
            drops.get(),
            3,
            "elements 0, 1, 2 must be dropped, nothing else"
        );
    }
}
