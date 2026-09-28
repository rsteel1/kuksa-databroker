# Chapter 26 hints

## c26_e01_raw_pointers
The compiler says E0133: dereferencing a raw pointer (and calling
`ptr.add`) is only allowed inside `unsafe`. Run `rustc --explain E0133`.

Make each `unsafe { }` block as small as possible: wrap the single
expression that needs it, not the whole function. Above each one, write
`// SAFETY:` and say why the pointer is valid *at that moment*: where it
came from, why it's in bounds, and why nobody else is mutating the data.

For `read_at`, the bounds check must come BEFORE `base.add(index)`. Even
*computing* an out-of-bounds pointer with `add` is UB. Almost the answer:
`if index >= values.len() { return None; }` then
`let value = unsafe { *base.add(index) };`.

## c26_e02_split_at_mut
The shipped version is E0499: two `&mut` borrows of `values`. The borrow
checker can't see that the halves don't overlap. You have to prove it
yourself with arithmetic.

Take `let ptr = values.as_mut_ptr();` once. Build the two slices with
`std::slice::from_raw_parts_mut(ptr, mid)` and
`std::slice::from_raw_parts_mut(ptr.add(mid), len - mid)`. The
`assert!(mid <= len, ...)` BEFORE the unsafe block is what makes the
function sound: without it, safe code could get out-of-bounds slices.

For `two_mut`: reject `i == j` and out-of-bounds indices first. Then split
at `max(i, j)`, so the smaller index is in the left half and the larger one
is `right[0]`. Return them in the caller's order.

## c26_e03_safe_abstraction_invariants
Everything hinges on one sentence: "`bytes` is always valid UTF-8". Each
method either establishes it (`from_bytes` validates), preserves it
(`push_str` appends a `&str`), or relies on it (`as_str`).

`as_str`: `unsafe { std::str::from_utf8_unchecked(&self.bytes) }`, with a
SAFETY comment that names the invariant and says why every mutator keeps it.

`truncate`: `str::is_char_boundary(new_len)` tells you whether cutting
there is safe. Check it BEFORE changing anything, and return
`Err(NotCharBoundary(new_len))` otherwise. Asking for a length ≥ the
current one is a no-op `Ok(())`.

`pop_segment`: `rfind('.')` on `self.as_str()`. '.' is a one-byte ASCII
character, so `dot` and `dot + 1` are always char boundaries. Copy the
segment out (`to_string()`), then `self.bytes.truncate(dot)`. With no dot,
the whole path is the segment and you truncate to 0.

## c26_e04_maybeuninit_and_layout
Create the guard first:
`let mut guard = Guard { slots: &mut slots, initialised: 0 };`. In the loop,
write the slot (`guard.slots[i].write(make(i))`) and only THEN set
`guard.initialised = i + 1`. If `make` panics, the guard's `Drop` runs
during unwinding.

`Guard::drop`: loop over `&mut self.slots[..self.initialised]` and call
`assume_init_drop()` on each (unsafe, SAFETY: those slots were written).

After the loop, `std::mem::forget(guard)`: the elements now belong to the
array you return. Then reinterpret:
`unsafe { std::ptr::read(slots.as_ptr().cast::<[T; N]>()) }`. That's sound
because `MaybeUninit<T>` has the same layout as `T` and every slot is
initialised. (Because of the borrow `&mut slots`, you can only use `slots`
again after the guard is forgotten.)

## c26_e05_ffi_call_c
`c_strlen`: `unsafe { strlen(s.as_ptr()) }`. A `&CStr` is exactly
"a valid pointer to a NUL-terminated buffer", which is strlen's contract.

`to_c_string`: `CString::new(text)` already returns
`Result<CString, NulError>`.

`c_abs`: check `x == i32::MIN` first and return `None`. C's `abs(INT_MIN)`
is undefined behaviour because `2147483648` doesn't fit in an `int`.

`sort_ids`: `qsort(ids.as_mut_ptr().cast(), ids.len(), size_of::<i32>(), Some(compare_i32))`.
Your SAFETY comment should mention the element size, the exclusive borrow,
and that the comparator matches the C ABI and never unwinds.

## c26_e06_ffi_expose_rust
Turning a maybe-null pointer into an optional reference is a one-liner:
`unsafe { store.as_mut() }` gives `Option<&mut SigStore>` (and `as_ref()`
gives `Option<&SigStore>` for `*const`). Use `let ... else` to return
`SIG_ERR_NULL`.

`sig_store_get`: check `out.is_null()` too, and only write through it
(`out.write(value)`) when the id exists, so an error leaves `*out` untouched.

`sig_store_for_each`: the visitor might unwind (it's `extern "C-unwind"`).
If that unwind escaped *your* `extern "C"` function, Rust would abort the
process. Wrap the loop in
`std::panic::catch_unwind(AssertUnwindSafe(|| { ... }))` and map `Err` to
`SIG_ERR_PANIC`.

`sig_store_free`: if non-null, `drop(unsafe { Box::from_raw(store) })`.
That's the only correct way to free memory created by `Box::into_raw`.

## c26_e07_ub_catalogue
None of the fixes need `unsafe`. That's the lesson: most UB in the wild
comes from reaching for unsafe where safe Rust already had an answer.

The rules broken, in order: dangling pointer to a freed local; pointer to a
temporary that is dropped at the end of the statement; invalid value (a
`bool` that isn't 0 or 1); misaligned and out-of-bounds read; two aliasing
`&mut`; a data race on `static mut`; a null reference.

Useful tools: `format!` (case 1). Keep the `CString` in a local while `f`
runs (case 2). `match` on the byte (case 3). `bytes.get(offset..end)?`
plus `try_into()` and `u32::from_le_bytes`, with `offset.checked_add(4)?`
to avoid overflow (case 4). Two separate `values[i] += 1` statements
(case 5). `AtomicU64::fetch_add` (case 6). `Iterator::find` plus `map`
(case 7).

## c26_e08_review_invariant_guards
Two bugs, both about `SignalPath`'s invariant ("`bytes` is always valid
UTF-8") not actually being enforced by the methods that are supposed to
guard it. Neither fix touches `as_str` itself.

Bug 1: `from_bytes` calls `std::str::from_utf8(&bytes)` just to throw the
result away with `let _ = ...`, so every byte string was accepted,
including invalid UTF-8. Replace it with `std::str::from_utf8(&bytes)?;`
so a `Utf8Error` actually stops construction.

Bug 2: `truncate` never checks whether `new_len` lands on a character
boundary before calling `self.bytes.truncate(new_len)`. Since the
invariant already guarantees `self.bytes` is valid UTF-8 at this point,
`self.as_str().is_char_boundary(new_len)` is a fully safe way to check —
return `Err(NotCharBoundary(new_len))` when it's `false`, before touching
`self.bytes`.
