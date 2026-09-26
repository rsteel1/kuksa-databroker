// c26_e02_split_at_mut.rs
//
// CONCEPT: building a safe API on top of unsafe code; "it compiles" vs "it is sound".
// GUIDE:   chapters/26-unsafe-and-ffi/guide.md  (section "Safe abstractions over unsafe code")
// DATABROKER: none directly. Compare `BytesMut::split_off` in the `bytes`
//             crate (used by tonic under the databroker), which splits one
//             buffer into two handles over disjoint regions.
// EXPECT:  compile error E0499 (cannot borrow `*values` as mutable more than once)
//
// The borrow checker reasons about *whole values*: two `&mut` borrows of the
// same slice are rejected even if you only touch disjoint halves. std's
// `slice::split_at_mut` is written with a small amount of unsafe code that
// proves disjointness by arithmetic and then hands out two safe `&mut [T]`s.
// Every caller stays in safe Rust.
//
// YOUR TASK
//   1. Implement `split_halves` with raw pointers and
//      `std::slice::from_raw_parts_mut` (don't call std's split_at_mut:
//      the point is to write it). Panic with a clear message if `mid > len`,
//      exactly like std does. Without that check, the function would be
//      *unsound*: safe callers could create out-of-bounds slices.
//   2. Implement `two_mut` in SAFE code using your `split_halves`.
//   Do not edit the tests.

#![allow(unsafe_code)]

/// Splits `values` into `[0, mid)` and `[mid, len)`.
///
/// # Panics
/// Panics if `mid > values.len()`.
fn split_halves<T>(values: &mut [T], mid: usize) -> (&mut [T], &mut [T]) {
    let len = values.len();
    assert!(mid <= len, "mid ({mid}) > len ({len})");
    let ptr = values.as_mut_ptr();
    // SAFETY: `mid <= len`, so `[ptr, ptr+mid)` and `[ptr+mid, ptr+len)` are
    // both inside the original allocation and do not overlap. The returned
    // slices borrow from `values` (same lifetime), so the original `&mut`
    // cannot be used while they exist: no aliasing `&mut` is ever created.
    unsafe {
        (
            std::slice::from_raw_parts_mut(ptr, mid),
            std::slice::from_raw_parts_mut(ptr.add(mid), len - mid),
        )
    }
}

/// Returns mutable references to two *different* elements, or `None` if
/// `i == j` or either index is out of bounds.
fn two_mut<T>(values: &mut [T], i: usize, j: usize) -> Option<(&mut T, &mut T)> {
    if i == j || i >= values.len() || j >= values.len() {
        return None;
    }
    // WHY: split at the larger index; the smaller one is in the left half.
    let (lo, hi) = (i.min(j), i.max(j));
    let (left, right) = split_halves(values, hi);
    let (a, b) = (&mut left[lo], &mut right[0]);
    Some(if i < j { (a, b) } else { (b, a) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_like_std() {
        let mut v = [1, 2, 3, 4, 5];
        let (a, b) = split_halves(&mut v, 2);
        a[0] = 10;
        b[0] = 30;
        assert_eq!(a, &[10, 2]);
        assert_eq!(b, &[30, 4, 5]);
        assert_eq!(v, [10, 2, 30, 4, 5]);
    }

    #[test]
    fn split_edges() {
        let mut v = [1, 2, 3];
        let (a, b) = split_halves(&mut v, 0);
        assert!(a.is_empty());
        assert_eq!(b.len(), 3);
        let (a, b) = split_halves(&mut v, 3);
        assert_eq!(a.len(), 3);
        assert!(b.is_empty());
        let mut empty: [u8; 0] = [];
        let (a, b) = split_halves(&mut empty, 0);
        assert!(a.is_empty() && b.is_empty());
    }

    #[test]
    #[should_panic(expected = "mid")]
    fn split_out_of_bounds_panics() {
        let mut v = [1, 2, 3];
        let _ = split_halves(&mut v, 4);
    }

    #[test]
    fn two_mut_swaps_order_correctly() {
        let mut v = vec![String::from("a"), String::from("b"), String::from("c")];
        {
            let (x, y) = two_mut(&mut v, 2, 0).expect("distinct indices");
            assert_eq!((x.as_str(), y.as_str()), ("c", "a"));
            std::mem::swap(x, y);
        }
        assert_eq!(v, ["c", "b", "a"]);
    }

    #[test]
    fn two_mut_rejects_bad_input() {
        let mut v = [1, 2, 3];
        assert!(two_mut(&mut v, 1, 1).is_none());
        assert!(two_mut(&mut v, 0, 3).is_none());
        assert!(two_mut(&mut v, 7, 0).is_none());
    }
}
