// c26_e05_ffi_call_c.rs
//
// CONCEPT: calling C from Rust: `extern "C"` declarations, `CStr`/`CString`,
//          C's own undefined behaviour, and wrapping it all in a safe API.
// GUIDE:   chapters/26-unsafe-and-ffi/guide.md  (section "Calling C: extern blocks and C strings")
// DATABROKER: databroker/src/main.rs:14-16 — with `--features jemalloc` the
//             binary swaps its allocator for jemalloc, a C library reached
//             through FFI by the `jemallocator` crate. The `ring` crypto
//             crate behind the `tls` feature is also C and assembly behind a
//             safe Rust API.
// EXPECT:  tests fail: todo!() panics
//
// On Linux every Rust binary already links the C standard library (glibc in
// the course devcontainer), so you can declare its functions yourself. The declaration is a *promise* to the
// compiler: get a type wrong and you get UB, not an error. (In real projects
// use the `libc` crate or generate declarations with `bindgen`.)
//
// Three C traps these wrappers must defend against:
//   * `strlen` walks memory until it finds a NUL byte, so it needs a real
//     NUL-terminated string: that's exactly what `&CStr` guarantees.
//   * `abs(INT_MIN)` is undefined behaviour in C (the result doesn't fit).
//   * `qsort` calls your comparator through a C function pointer. A Rust
//     panic must never unwind into C code.
//
// YOUR TASK
//   Implement the four safe wrappers. Each `unsafe` block needs a `// SAFETY:`
//   comment. `sort_ids` must use `qsort` with the provided `compare_i32`.
//   Do not edit the extern block or the tests.

#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, c_void, CStr, CString, NulError};

extern "C" {
    fn strlen(s: *const c_char) -> usize;
    fn abs(x: c_int) -> c_int;
    fn qsort(
        base: *mut c_void,
        count: usize,
        size: usize,
        compare: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    );
}

/// C-ABI comparator for two `i32`s. It cannot panic.
unsafe extern "C" fn compare_i32(a: *const c_void, b: *const c_void) -> c_int {
    // SAFETY: `qsort` only passes pointers to elements of the array handed to
    // it by `sort_ids`, which is a `[i32]`, so both are valid aligned i32s.
    let (a, b) = unsafe { (*a.cast::<i32>(), *b.cast::<i32>()) };
    a.cmp(&b) as c_int
}

/// Length in bytes of a C string, computed by C's `strlen`.
fn c_strlen(s: &CStr) -> usize {
    // SAFETY: a `&CStr` always points to a valid NUL-terminated buffer that
    // stays alive for the duration of the borrow, which is what strlen requires.
    unsafe { strlen(s.as_ptr()) }
}

/// Builds a C string from Rust text. Fails if `text` contains a NUL byte,
/// because C would see the string end early.
fn to_c_string(text: &str) -> Result<CString, NulError> {
    CString::new(text)
}

/// C's `abs`, made safe: `None` where C would have undefined behaviour.
fn c_abs(x: i32) -> Option<i32> {
    if x == i32::MIN {
        return None;
    }
    // SAFETY: `abs` is a pure libc function; for any input except INT_MIN
    // (excluded above) its result is representable, so there's no UB.
    Some(unsafe { abs(x) })
}

/// Sorts signal ids in place using C's `qsort`.
fn sort_ids(ids: &mut [i32]) {
    // SAFETY: `ids.as_mut_ptr()` is valid for `ids.len()` contiguous i32s of
    // `size_of::<i32>()` bytes each, exclusively borrowed for this call.
    // `compare_i32` matches the comparator ABI, reads only those elements,
    // and never unwinds.
    unsafe {
        qsort(
            ids.as_mut_ptr().cast::<c_void>(),
            ids.len(),
            std::mem::size_of::<i32>(),
            Some(compare_i32),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strlen_counts_bytes_not_chars() {
        let s = to_c_string("Vehicle.Speed").expect("no NUL");
        assert_eq!(c_strlen(&s), 13);
        let s = to_c_string("Tür").expect("no NUL");
        assert_eq!(c_strlen(&s), 4, "ü is two bytes in UTF-8");
        assert_eq!(c_strlen(c"literal"), 7, "c\"...\" literals are &CStr");
    }

    #[test]
    fn interior_nul_is_rejected() {
        assert!(to_c_string("Vehicle\0Speed").is_err());
    }

    #[test]
    fn abs_is_safe_everywhere() {
        assert_eq!(c_abs(-42), Some(42));
        assert_eq!(c_abs(7), Some(7));
        assert_eq!(c_abs(i32::MAX), Some(i32::MAX));
        assert_eq!(c_abs(i32::MIN + 1), Some(i32::MAX));
        assert_eq!(c_abs(i32::MIN), None);
    }

    #[test]
    fn qsort_sorts() {
        let mut ids = [42, -7, 0, 1_000_000, -7, 3];
        sort_ids(&mut ids);
        assert_eq!(ids, [-7, -7, 0, 3, 42, 1_000_000]);
        let mut empty: [i32; 0] = [];
        sort_ids(&mut empty);
    }
}
