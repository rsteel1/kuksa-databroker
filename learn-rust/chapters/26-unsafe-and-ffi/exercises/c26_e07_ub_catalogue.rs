// c26_e07_ub_catalogue.rs
//
// CONCEPT: recognising Undefined Behaviour, and fixing it, usually by
//          *removing* the unsafe code.
// GUIDE:   chapters/26-unsafe-and-ffi/guide.md  (section "A catalogue of undefined behaviour")
// DATABROKER: the whole databroker, which contains zero `unsafe` blocks
//             (`grep -rn "unsafe" databroker/src`). Every case below has a
//             safe rewrite with the same performance, which is why the
//             project never needed any.
// EXPECT:  tests fail: todo!() panics
//
// Each function below has an UNSOUND version in its doc comment: code that
// compiles (with `unsafe`) but has undefined behaviour. None of it is ever
// executed. UB means the compiler may assume it never happens, so the program
// can do *anything*: often nothing visible, until a different optimisation
// level or platform turns it into a crash or silent corruption.
//
// YOUR TASK
//   For each case: read the unsound snippet, name the rule it breaks (write
//   it in the `// BROKEN RULE:` comment), then implement the SOUND version
//   in safe Rust. No `unsafe` is needed anywhere in this file.
//   Do not edit the tests.

// I AM NOT DONE

use std::ffi::CStr;
use std::sync::atomic::{AtomicU64, Ordering};

/// CASE 1 — returning a pointer into a local.
/// ```ignore
/// fn make_label(id: u32) -> *const u8 {
///     let s = format!("sig-{id}");
///     s.as_ptr() // `s` is freed when the function returns
/// }
/// ```
// BROKEN RULE: ???
fn make_label(id: u32) -> String {
    let _ = id;
    todo!()
}

/// CASE 2 — pointer to a temporary.
/// ```ignore
/// let p = CString::new(path).unwrap().as_ptr(); // CString dropped here!
/// unsafe { strlen(p) }                          // reads freed memory
/// ```
// BROKEN RULE: ???
/// Sound version: keep the `CString` alive for as long as it is borrowed.
/// Returns `None` if `path` contains a NUL byte.
fn with_c_path<R>(path: &str, f: impl FnOnce(&CStr) -> R) -> Option<R> {
    let _ = (path, f);
    todo!()
}

/// CASE 3 — invalid bit pattern.
/// ```ignore
/// let flag: bool = unsafe { std::mem::transmute(byte) }; // byte = 2 is UB
/// ```
// BROKEN RULE: ???
fn flag_from_byte(byte: u8) -> Option<bool> {
    let _ = byte;
    todo!()
}

/// CASE 4 — unaligned and out-of-bounds read.
/// ```ignore
/// let v = unsafe { *(bytes.as_ptr().add(offset) as *const u32) };
/// ```
// BROKEN RULE: ???
/// Sound version: little-endian u32 at `offset`, `None` if out of bounds.
fn read_u32_le_at(bytes: &[u8], offset: usize) -> Option<u32> {
    let _ = (bytes, offset);
    todo!()
}

/// CASE 5 — two live `&mut` to the same place.
/// ```ignore
/// let p = values.as_mut_ptr();
/// let a = unsafe { &mut *p.add(i) };
/// let b = unsafe { &mut *p.add(j) }; // if i == j: two &mut alias = UB
/// *a += 1; *b += 1;
/// ```
// BROKEN RULE: ???
/// Sound version: increment `values[i]` and `values[j]` (twice if `i == j`).
/// Panics on out-of-bounds indices, like normal indexing.
fn bump_both(values: &mut [u32], i: usize, j: usize) {
    let _ = (values, i, j);
    todo!()
}

/// CASE 6 — data race on a `static mut`.
/// ```ignore
/// static mut NEXT_ID: u64 = 0;
/// fn next_id() -> u64 { unsafe { NEXT_ID += 1; NEXT_ID } } // racy from 2 threads
/// ```
// BROKEN RULE: ???
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Sound version: unique, increasing ids from any thread.
fn next_id() -> u64 {
    let _ = (&NEXT_ID, Ordering::Relaxed);
    todo!()
}

/// CASE 7 — null "reference".
/// ```ignore
/// fn find(signals: &[(i32, String)], id: i32) -> &str {
///     let p = signals.iter().find(|s| s.0 == id).map_or(std::ptr::null(), |s| &s.1 as *const String);
///     unsafe { &*p } // null when not found
/// }
/// ```
// BROKEN RULE: ???
fn find_signal(signals: &[(i32, String)], id: i32) -> Option<&str> {
    let _ = (signals, id);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case1_owned_label() {
        assert_eq!(make_label(7), "sig-7");
    }

    #[test]
    fn case2_cstring_lives_long_enough() {
        let len = with_c_path("Vehicle.Speed", |c| c.to_bytes().len());
        assert_eq!(len, Some(13));
        assert_eq!(with_c_path("bad\0path", |_| ()), None);
    }

    #[test]
    fn case3_bool_validation() {
        assert_eq!(flag_from_byte(0), Some(false));
        assert_eq!(flag_from_byte(1), Some(true));
        assert_eq!(flag_from_byte(2), None);
        assert_eq!(flag_from_byte(255), None);
    }

    #[test]
    fn case4_unaligned_reads() {
        let bytes = [0xAA, 0x78, 0x56, 0x34, 0x12, 0xBB];
        assert_eq!(read_u32_le_at(&bytes, 1), Some(0x1234_5678));
        assert_eq!(read_u32_le_at(&bytes, 2), Some(0xBB12_3456));
        assert_eq!(read_u32_le_at(&bytes, 3), None);
        assert_eq!(read_u32_le_at(&bytes, usize::MAX), None);
    }

    #[test]
    fn case5_aliasing() {
        let mut v = [0, 10, 20];
        bump_both(&mut v, 0, 2);
        assert_eq!(v, [1, 10, 21]);
        bump_both(&mut v, 1, 1);
        assert_eq!(v, [1, 12, 21]);
    }

    #[test]
    fn case6_ids_are_unique_across_threads() {
        let mut ids: Vec<u64> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..4)
                .map(|_| s.spawn(|| (0..1000).map(|_| next_id()).collect::<Vec<_>>()))
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("no panic"))
                .collect()
        });
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "every id must be unique");
        assert!(ids[0] >= 1);
    }

    #[test]
    fn case7_option_not_null() {
        let signals = vec![(1, String::from("Vehicle.Speed"))];
        assert_eq!(find_signal(&signals, 1), Some("Vehicle.Speed"));
        assert_eq!(find_signal(&signals, 2), None);
    }
}
