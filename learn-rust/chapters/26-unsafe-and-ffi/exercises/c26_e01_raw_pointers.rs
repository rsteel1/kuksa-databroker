// c26_e01_raw_pointers.rs
//
// CONCEPT: raw pointers (`*const T`, `*mut T`), `unsafe` blocks, `// SAFETY:` comments.
// GUIDE:   chapters/26-unsafe-and-ffi/guide.md  (section "Raw pointers and the unsafe keyword")
// DATABROKER: none — `grep -rn unsafe databroker/src` finds no unsafe code at all.
//             The unsafe lives one layer down, in dependencies such as `bytes`
//             (BytesMut::split_off) and tokio's channel blocks.
// EXPECT:  compile error E0133 (dereference of raw pointer is unsafe and
//          requires unsafe function or block)
//
// Creating a raw pointer is safe. *Dereferencing* one is not, because the
// compiler can no longer prove it points at valid, aligned, initialised
// memory that nobody else is mutating. `unsafe { ... }` doesn't switch the
// borrow checker off. It lets you do five extra things (deref raw pointers
// is one), and makes YOU responsible for proving they're OK. Write that
// proof down in a `// SAFETY:` comment directly above the block.
//
// YOUR TASK
//   1. Make the file compile by putting each raw-pointer dereference inside
//      the smallest possible `unsafe { }` block, with a `// SAFETY:` comment
//      saying why the pointer is valid at that moment.
//   2. `read_at` must never dereference an out-of-bounds pointer: return
//      `None` for a bad index (check BEFORE creating the offset pointer).
//   Do not edit the tests.

// I AM NOT DONE

#![allow(unsafe_code)]

/// Reads element `index` of `values` through a raw pointer.
fn read_at(values: &[u32], index: usize) -> Option<u32> {
    let base: *const u32 = values.as_ptr();
    let value = *base.add(index);
    Some(value)
}

/// Doubles the value behind `target` via a `*mut` pointer.
fn double_in_place(target: &mut i64) {
    let ptr: *mut i64 = target;
    *ptr *= 2;
}

/// Swaps two values using raw pointers (what `std::ptr::swap` does inside).
fn swap_raw(a: &mut String, b: &mut String) {
    let pa: *mut String = a;
    let pb: *mut String = b;
    std::ptr::swap(pa, pb);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_in_bounds() {
        let v = [10, 20, 30];
        assert_eq!(read_at(&v, 0), Some(10));
        assert_eq!(read_at(&v, 2), Some(30));
    }

    #[test]
    fn rejects_out_of_bounds() {
        let v = [10, 20, 30];
        assert_eq!(read_at(&v, 3), None);
        assert_eq!(read_at(&v, usize::MAX), None);
        assert_eq!(read_at(&[], 0), None);
    }

    #[test]
    fn doubles() {
        let mut x = 21;
        double_in_place(&mut x);
        assert_eq!(x, 42);
    }

    #[test]
    fn swaps_without_cloning() {
        let mut a = String::from("Vehicle.Speed");
        let mut b = String::from("Vehicle.Cabin.Temperature");
        let a_buffer = a.as_ptr();
        swap_raw(&mut a, &mut b);
        assert_eq!(a, "Vehicle.Cabin.Temperature");
        assert_eq!(b, "Vehicle.Speed");
        // The heap buffer moved with the String: no copy was made.
        assert_eq!(b.as_ptr(), a_buffer);
    }
}
