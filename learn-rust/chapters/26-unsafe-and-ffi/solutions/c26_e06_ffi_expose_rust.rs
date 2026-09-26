// c26_e06_ffi_expose_rust.rs
//
// CONCEPT: exposing Rust to C: `extern "C" fn`, `#[no_mangle]`, `#[repr(C)]`,
//          opaque handles (`Box::into_raw`/`Box::from_raw`), error codes,
//          callbacks with a `user_data` pointer, and stopping panics at the
//          boundary (`catch_unwind`, the `C-unwind` ABI).
// GUIDE:   chapters/26-unsafe-and-ffi/guide.md  (section "Exposing Rust to C: handles, repr(C) and callbacks")
// DATABROKER: databroker/src/broker.rs — `Database` (id -> value store). Many
//             in-vehicle ECUs run C/C++, so "a C API over a Rust signal store"
//             is exactly the shape you'd build to embed databroker logic there.
// EXPECT:  tests fail: the stubs return SIG_TODO (-99)
//
// A C caller can't see Rust types, so it gets an *opaque handle*: a pointer
// it must pass back to every call and eventually hand to `sig_store_free`.
// The rules:
//   * Every function that dereferences a pointer argument is an
//     `unsafe extern "C" fn` with a `# Safety` doc section: the caller (C)
//     must uphold it.
//   * Null pointers are reported with error codes, never dereferenced.
//   * Panics must not unwind into C: wrap bodies in `catch_unwind`.
//   * Whoever allocates frees: memory from `Box::into_raw` is freed ONLY by
//     `Box::from_raw`, never by C's `free`.
//
// YOUR TASK
//   Replace the `SIG_TODO` stubs: `sig_store_set`, `sig_store_get`,
//   `sig_store_for_each` and `sig_store_free`. `sig_store_new` is provided as
//   a model. Tests call the functions exactly as C code would.
//   Do not edit the tests.

#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::{c_int, c_void};

pub const SIG_OK: c_int = 0;
pub const SIG_ERR_NULL: c_int = -1;
pub const SIG_ERR_NOT_FOUND: c_int = -2;
pub const SIG_ERR_PANIC: c_int = -3;

/// A datapoint laid out exactly like the C struct
/// `struct SigDatapoint { int32_t id; double value; };`
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SigDatapoint {
    pub id: i32,
    pub value: f64,
}

/// Opaque to C: it only ever sees `SigStore *`.
#[derive(Default)]
pub struct SigStore {
    values: BTreeMap<i32, f64>,
}

/// Callback type: `void (*)(struct SigDatapoint dp, void *user_data)`.
///
/// It is `extern "C-unwind"` rather than `extern "C"` because we want to
/// accept callbacks that might unwind (a Rust closure trampoline, or C++ code
/// that throws). A panic escaping a plain `extern "C"` function aborts the
/// whole process (Rust 1.81+). With `C-unwind` the unwind reaches us, and we
/// stop it before it can reach *our* C caller.
pub type SigVisitor = extern "C-unwind" fn(dp: SigDatapoint, user_data: *mut c_void);

/// Creates a store. Free it with `sig_store_free`.
#[no_mangle]
pub extern "C" fn sig_store_new() -> *mut SigStore {
    Box::into_raw(Box::default())
}

/// Sets `id` to `value`. Returns `SIG_OK` or `SIG_ERR_NULL`.
///
/// # Safety
/// `store` must be null or a pointer returned by `sig_store_new` that has
/// not been freed, and must not be used concurrently from another thread.
#[no_mangle]
pub unsafe extern "C" fn sig_store_set(store: *mut SigStore, id: i32, value: f64) -> c_int {
    // SAFETY: per the contract above, a non-null `store` came from
    // `Box::into_raw` and is live and unaliased for the duration of this call.
    let Some(store) = (unsafe { store.as_mut() }) else {
        return SIG_ERR_NULL;
    };
    store.values.insert(id, value);
    SIG_OK
}

/// Writes the value of `id` into `*out`. Returns `SIG_OK`, `SIG_ERR_NULL`
/// or `SIG_ERR_NOT_FOUND` (in which case `*out` is left untouched).
///
/// # Safety
/// `store` as for `sig_store_set`; `out` must be null or valid for writing an `f64`.
#[no_mangle]
pub unsafe extern "C" fn sig_store_get(store: *const SigStore, id: i32, out: *mut f64) -> c_int {
    if out.is_null() {
        return SIG_ERR_NULL;
    }
    // SAFETY: non-null `store` is a live handle from `sig_store_new` (contract).
    let Some(store) = (unsafe { store.as_ref() }) else {
        return SIG_ERR_NULL;
    };
    match store.values.get(&id) {
        Some(value) => {
            // SAFETY: `out` is non-null (checked) and the caller promised it
            // is valid and aligned for an f64 write.
            unsafe { out.write(*value) };
            SIG_OK
        }
        None => SIG_ERR_NOT_FOUND,
    }
}

/// Calls `visit(dp, user_data)` for every datapoint in id order.
/// Returns `SIG_OK`, `SIG_ERR_NULL`, or `SIG_ERR_PANIC` if the visitor
/// panicked (the panic must NOT unwind into the C caller).
///
/// # Safety
/// `store` as for `sig_store_set`. `user_data` is passed through untouched.
#[no_mangle]
pub unsafe extern "C" fn sig_store_for_each(
    store: *const SigStore,
    visit: Option<SigVisitor>,
    user_data: *mut c_void,
) -> c_int {
    let Some(visit) = visit else {
        return SIG_ERR_NULL;
    };
    // SAFETY: non-null `store` is a live handle from `sig_store_new` (contract).
    let Some(store) = (unsafe { store.as_ref() }) else {
        return SIG_ERR_NULL;
    };
    use std::panic::{catch_unwind, AssertUnwindSafe};
    // WHY: the visitor may unwind (C-unwind ABI). If that unwind escaped this
    // `extern "C"` function the process would abort, so catch it and turn it
    // into an error code instead.
    let result = catch_unwind(AssertUnwindSafe(|| {
        for (&id, &value) in &store.values {
            visit(SigDatapoint { id, value }, user_data);
        }
    }));
    match result {
        Ok(()) => SIG_OK,
        Err(_) => SIG_ERR_PANIC,
    }
}

/// Frees a store. Passing null is a no-op, like C's `free(NULL)`.
///
/// # Safety
/// `store` must be null or a pointer from `sig_store_new` that has not been
/// freed yet. It must not be used after this call.
#[no_mangle]
pub unsafe extern "C" fn sig_store_free(store: *mut SigStore) {
    if !store.is_null() {
        // SAFETY: the pointer came from `Box::into_raw` in `sig_store_new`
        // and, per the contract, is freed exactly once.
        drop(unsafe { Box::from_raw(store) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    extern "C-unwind" fn collect(dp: SigDatapoint, user_data: *mut c_void) {
        // SAFETY: the tests always pass a `*mut Vec<SigDatapoint>` as user_data.
        let out = unsafe { &mut *user_data.cast::<Vec<SigDatapoint>>() };
        out.push(dp);
    }

    extern "C-unwind" fn explode(_dp: SigDatapoint, _user_data: *mut c_void) {
        panic!("visitor bug");
    }

    #[test]
    fn repr_c_layout_matches_c() {
        // int32_t (4) + padding (4) + double (8)
        assert_eq!(std::mem::size_of::<SigDatapoint>(), 16);
        assert_eq!(std::mem::align_of::<SigDatapoint>(), 8);
    }

    #[test]
    fn set_get_round_trip() {
        let store = sig_store_new();
        let mut out = 0.0;
        // SAFETY: `store` is live until freed at the end; `out` is a valid f64.
        unsafe {
            assert_eq!(sig_store_set(store, 7, 88.5), SIG_OK);
            assert_eq!(sig_store_get(store, 7, &mut out), SIG_OK);
            assert_eq!(out, 88.5);
            out = -1.0;
            assert_eq!(sig_store_get(store, 8, &mut out), SIG_ERR_NOT_FOUND);
            assert_eq!(out, -1.0, "out untouched on error");
            sig_store_free(store);
        }
    }

    #[test]
    fn null_pointers_are_errors_not_crashes() {
        let mut out = 0.0;
        // SAFETY: null is explicitly allowed by every function's contract.
        unsafe {
            assert_eq!(sig_store_set(ptr::null_mut(), 1, 1.0), SIG_ERR_NULL);
            assert_eq!(sig_store_get(ptr::null(), 1, &mut out), SIG_ERR_NULL);
            let store = sig_store_new();
            assert_eq!(sig_store_get(store, 1, ptr::null_mut()), SIG_ERR_NULL);
            assert_eq!(
                sig_store_for_each(store, None, ptr::null_mut()),
                SIG_ERR_NULL
            );
            sig_store_free(store);
            sig_store_free(ptr::null_mut());
        }
    }

    #[test]
    fn callback_with_user_data_in_id_order() {
        let store = sig_store_new();
        let mut seen: Vec<SigDatapoint> = Vec::new();
        // SAFETY: live store; user_data points at `seen`, which outlives the call.
        unsafe {
            sig_store_set(store, 3, 30.0);
            sig_store_set(store, 1, 10.0);
            let rc = sig_store_for_each(store, Some(collect), (&mut seen as *mut Vec<_>).cast());
            assert_eq!(rc, SIG_OK);
            sig_store_free(store);
        }
        assert_eq!(
            seen,
            [
                SigDatapoint { id: 1, value: 10.0 },
                SigDatapoint { id: 3, value: 30.0 }
            ]
        );
    }

    #[test]
    fn panicking_visitor_does_not_unwind_into_c() {
        let store = sig_store_new();
        // SAFETY: live store; the visitor ignores user_data.
        unsafe {
            sig_store_set(store, 1, 1.0);
            assert_eq!(
                sig_store_for_each(store, Some(explode), ptr::null_mut()),
                SIG_ERR_PANIC
            );
            sig_store_free(store);
        }
    }
}
