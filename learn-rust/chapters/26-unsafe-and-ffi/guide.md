# 26 — Unsafe Rust & FFI

> **Builds on:** chapters 02 (ownership), 11 (lifetimes), 12 (Box, Drop), 15 (memory layout).

**Why this matters for the databroker, and beyond.** The databroker has
**zero** `unsafe` blocks (`grep -rn unsafe databroker/src` finds only the
word inside other identifiers). Yet it runs on code full of `unsafe`:
tokio's channels, `bytes`, hashbrown, the `ring` crypto library behind
`tls`, and the optional jemalloc allocator are all built on unsafe Rust or C.
To read any systems crate, review a dependency, or call a C library from a
vehicle ECU, you need to know what `unsafe` promises, what it doesn't, and
how to spot code that's wrong even though it compiles. This chapter targets
the course's Linux devcontainer (Debian, glibc, x86_64 or aarch64).

---

## Raw pointers and the unsafe keyword

Safe Rust guarantees no dangling references, no data races and no invalid
values. It does this by rejecting some *correct* programs it can't prove
correct. `unsafe` is how you say "I have proved it myself". It does **not**
turn off the borrow checker, type checking or anything else. It unlocks
exactly five extra operations:

1. dereference a raw pointer (`*const T`, `*mut T`);
2. call an `unsafe fn` (including foreign C functions);
3. access or modify a `static mut`;
4. implement an `unsafe trait` (`Send`, `Sync`, `GlobalAlloc`, ...);
5. access fields of a `union`.

Creating a raw pointer is safe (`let p = &x as *const i32;`). Using it is
not, because the compiler can't know whether it is:

| Requirement | Meaning |
|---|---|
| non-null | points somewhere |
| aligned | address is a multiple of `align_of::<T>()` |
| dereferenceable | the whole `size_of::<T>()` bytes are inside one live allocation |
| initialised | holds a valid `T` (a `bool` must be 0 or 1, a reference non-null, ...) |
| not aliased wrongly | no `&mut` to it exists at the same time as another reference |

**Conventions you'll see in every good crate:**

```rust
// SAFETY: <why every requirement above holds right here>
let value = unsafe { *ptr };

/// # Safety
/// `ptr` must be non-null, aligned and point to an initialised `u32`.
unsafe fn read_raw(ptr: *const u32) -> u32 { unsafe { *ptr } }
```

A `// SAFETY:` comment justifies an `unsafe` *block* (the author took on the
proof obligation). A `# Safety` doc section on an `unsafe fn` states what the
*caller* must prove. Keep blocks tiny, so the proof covers one expression.

Lints that enforce this (all used in this course):

| Lint | Effect |
|---|---|
| `unsafe_code = "forbid"` / `"deny"` | ban `unsafe` in a crate (the databroker could safely set `forbid`) |
| `clippy::undocumented_unsafe_blocks` | every `unsafe {}` needs a `// SAFETY:` comment (on for `solutions/`) |
| `unsafe_op_in_unsafe_fn` | the body of an `unsafe fn` still needs `unsafe {}` blocks (warn by default in edition 2024) |

## Safe abstractions over unsafe code

A function is **sound** if *no safe caller, whatever arguments it passes,
can cause undefined behaviour*. "It compiles and the tests pass" isn't the
test. `split_at_mut` is the classic example:

```rust
fn split_at_mut<T>(v: &mut [T], mid: usize) -> (&mut [T], &mut [T]) {
    let len = v.len();
    assert!(mid <= len);                   // <- this line makes it sound
    let p = v.as_mut_ptr();
    // SAFETY: the two ranges are disjoint and inside `v`.
    unsafe {
        (std::slice::from_raw_parts_mut(p, mid),
         std::slice::from_raw_parts_mut(p.add(mid), len - mid))
    }
}
```

Delete the `assert!` and the function still compiles, still passes every
test with valid input, and is **unsound**: `split_at_mut(&mut v, 99)` would
hand safe code a slice into memory it doesn't own. The unsafe block
is fine. The bug is the missing precondition check in the *safe* part.

Unsafe code is a **module-level** property. The proof inside one `unsafe`
block often depends on code elsewhere in the same module, which brings us
to invariants.

## Invariants: why privacy is a safety feature

`String` is a `Vec<u8>` plus the promise "always valid UTF-8", and
`String::as_str` relies on it without re-checking. If `String`'s field
were `pub`, *safe* code could write `0xFF` into it and `as_str` would
produce an invalid `&str`, which is UB, with no `unsafe` in sight at the
point where the damage was done.

So:

- **State the invariant** in a comment on the type (`INVARIANT: ...`).
- **Keep the fields private** so only your module can touch them.
- **Audit every method, safe ones included.** Each must establish or
  preserve the invariant. A safe `truncate` that cuts inside a
  multi-byte character would break `String` as surely as bad unsafe code.

The same design without unsafe is the validated newtype (chapter 03).
`databroker/src/glob.rs:29` `Matcher` keeps `glob_pattern` private and
builds it only through `Matcher::new`. There, breaking the invariant gives
wrong matches. With `unsafe` behind it, it gives memory corruption.

## Uninitialised memory: MaybeUninit

`MaybeUninit<T>` is "space for a `T` that may not hold one yet". The
compiler never assumes it's valid, never drops it, and lets you:

```rust
let mut slot = MaybeUninit::<String>::uninit();
slot.write(String::from("Vehicle.Speed"));     // safe: just writes
let s = unsafe { slot.assume_init() };          // you promise it's written
```

Rules of thumb:

- Never create a `&T` or `&mut T` to uninitialised memory. Use
  `MaybeUninit::write` or raw-pointer writes.
- `mem::uninitialized()` and `mem::zeroed()` for types with invalid bit
  patterns (references, `bool`, enums) are UB. They're deprecated for that
  reason.
- **Panic safety:** if code can panic halfway through initialisation,
  something must drop the parts already built *and only those*. The
  standard trick is a guard struct whose `Drop` cleans up, which you
  `mem::forget` on success (exercise e04). `std::array::from_fn` and
  `Vec::extend` work this way internally.

In the wild: tokio's mpsc channel stores message slots as
`[UnsafeCell<MaybeUninit<T>>; BLOCK_CAP]`
(`tokio-1.53.1/src/sync/mpsc/block.rs:47`) and writes each with
`ptr::write(ptr, MaybeUninit::new(value))` (`block.rs:208`).

## Calling C: extern blocks and C strings

```rust
use std::ffi::{c_char, CStr};

extern "C" {
    fn strlen(s: *const c_char) -> usize;   // a promise about the signature
}

let s: &CStr = c"Vehicle.Speed";              // C-string literal (Rust 1.77+)
let n = unsafe { strlen(s.as_ptr()) };        // SAFETY: CStr is NUL-terminated
```

- An `extern` block **declares** functions. The declaration is a promise:
  if the types don't match the C side, the result is UB, not a compile
  error. In real projects, take declarations from the `libc` crate
  (`libc-0.2.189/src/unix/mod.rs:753` declares `strlen` and `:640` `qsort`)
  or generate them with **bindgen** from the C headers.
- On Linux, every Rust binary links the C library (glibc in the course
  devcontainer). Any other library needs a `#[link(name = "...")]` or a
  `build.rs` that prints `cargo:rustc-link-lib=...`.
- Edition 2024 requires `unsafe extern "C" { ... }`, which makes the
  "trust me" explicit.

**Strings.** Rust `&str` is UTF-8 with a length and no terminator. C
`char *` is NUL-terminated bytes with no encoding guarantee. Convert
explicitly:

| Direction | Type | Notes |
|---|---|---|
| Rust → C, owned | `CString::new(s)?` | fails if `s` contains `\0`; keep the `CString` alive while C uses the pointer |
| C → Rust, borrowed | `unsafe { CStr::from_ptr(p) }` | then `.to_str()?` (UTF-8 check) or `.to_string_lossy()` |

The most common FFI bug in the wild is
`CString::new(x).unwrap().as_ptr()`: the temporary `CString` is dropped at
the end of the statement, leaving a dangling pointer (exercise e07, case 2).

**C's own UB** doesn't disappear when called from Rust. `abs(INT_MIN)` is
undefined in C, so a safe wrapper must exclude it (e05).

## Exposing Rust to C: handles, repr(C) and callbacks

```rust
#[repr(C)]                        // C field order and C padding rules
pub struct SigDatapoint { pub id: i32, pub value: f64 }

#[no_mangle]                      // keep the symbol name "sig_store_new"
pub extern "C" fn sig_store_new() -> *mut SigStore {
    Box::into_raw(Box::default()) // ownership goes to C...
}

#[no_mangle]
pub unsafe extern "C" fn sig_store_free(p: *mut SigStore) {
    if !p.is_null() { drop(unsafe { Box::from_raw(p) }) } // ...and comes back
}
```

- **Layout.** Without `#[repr(C)]` Rust may reorder fields. With it, the
  struct matches what a C compiler produces for the same target. On both
  x86_64 and aarch64 Linux, `SigDatapoint` is 16 bytes with 8-byte
  alignment: 4 bytes of padding follow `id`.
- **Opaque handles.** C never sees inside `SigStore`, only a pointer.
  Allocation and freeing must both happen on the Rust side: never `free()`
  memory created by `Box` (Rust's allocator may not be malloc, for example
  with jemalloc; see `databroker/src/main.rs:14-16`).
- **Errors.** Return integer codes, check every pointer for null, and
  leave out-parameters untouched on error.
- **Panics.** Unwinding out of an `extern "C"` function aborts the
  process (Rust 1.81+). Wrap bodies that can panic in
  `std::panic::catch_unwind`. Callbacks that may unwind (a Rust
  trampoline, or C++ throwing) use the `extern "C-unwind"` ABI, so the
  unwind can reach you and be stopped there.
- **Callbacks** take a `void *user_data` so C can pass context through,
  the C version of a closure capture.
- **cbindgen** generates the C header from these declarations.

## A catalogue of undefined behaviour

UB isn't "crashes". It means "the compiler assumed this never happens".
Code after a UB point can be optimised in ways that look impossible, can
differ between `debug` and `release`, and can change with the next compiler
upgrade. The common kinds (from the Nomicon and the Reference):

| UB | Typical cause | Safe replacement |
|---|---|---|
| dangling pointer | pointer to a local or a freed buffer | return owned data (`String`, `Vec`) |
| pointer to a temporary | `CString::new(x)?.as_ptr()` | bind the owner to a variable first |
| invalid value | `transmute::<u8, bool>(2)`, a null reference, an enum out of range | `match`/`TryFrom` validation |
| misaligned or out-of-bounds access | `*(bytes.as_ptr().add(i) as *const u32)` | `from_le_bytes` + `slice::get` |
| aliasing `&mut` | two `&mut` to the same element | sequential indexing, `split_at_mut` |
| data race | `static mut` counter | atomics, `Mutex`, channels |
| reading uninitialised memory | `mem::uninitialized()` | `MaybeUninit`, `Vec::with_capacity` |
| breaking a library invariant | writing non-UTF-8 into a `str` | keep invariants behind private fields |

**Tools on Linux:**

```sh
rustup toolchain install nightly --component miri
cargo +nightly miri test -p solutions --test c26_e02_split_at_mut
```

**Miri** interprets your program and reports most UB above: aliasing
violations (Stacked or Tree Borrows), use-after-free, misaligned reads and
invalid values. It can't run most foreign C calls (e05's `qsort`), so test
FFI with sanitizers instead:
`RUSTFLAGS="-Zsanitizer=address" cargo +nightly test --target x86_64-unknown-linux-gnu`
(use `aarch64-unknown-linux-gnu` on ARM) or `valgrind`.

**`no_std` and embedded.** On a microcontroller there's no OS and often no
allocator. `#![no_std]` crates use `core` (and optionally `alloc`), and
hardware registers are touched through `unsafe` volatile pointer reads and
writes, wrapped in safe HAL crates exactly as this chapter describes. The
in-vehicle world increasingly uses this, so recognising the pattern pays off.

## Mental model

| Idea | Analogy |
|---|---|
| `unsafe` block | Signing a contract: "I checked the rules the compiler couldn't" |
| `// SAFETY:` comment | The proof attached to the signature, for reviewers |
| `unsafe fn` + `# Safety` | A contract the *caller* must sign |
| Soundness | No safe caller can make it misbehave, however hard they try |
| Invariant behind private fields | A vault that only your module has the combination to |
| FFI declaration | Trusting a phone number someone wrote on a napkin |

The guarantee: **if every unsafe block is sound, safe Rust's guarantees
hold for the entire program.** A bug in safe code can't cause UB unless
some unsafe code relied on it. That's why unsafe is kept small, commented
and audited.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "`unsafe` turns off the borrow checker." | It unlocks five operations; everything else is still checked. |
| "If it doesn't crash, there's no UB." | UB is often silent until another optimisation level or compiler version. Use Miri. |
| "Unsafe code is the only place bugs can cause UB." | Safe code that breaks an invariant (a pub field, a bad `truncate`) makes *sound-looking* unsafe code UB. |
| "E0133 means I should add `unsafe`." | First ask whether a safe API already does it: `split_at_mut`, `from_le_bytes`, `get`, atomics. |
| "`extern "C"` functions can panic like normal ones." | Unwinding out of `extern "C"` aborts; catch it or use `C-unwind`. |
| "`&CStr` and `&str` are interchangeable." | NUL-terminated bytes vs UTF-8 with length; convert explicitly. |

## Smells and idioms

- **`unsafe` for performance without a measurement.** Chapter 27 shows the
  safe versions (iterators, `from_le_bytes`) usually compile to the same
  machine code.
- **Large `unsafe` blocks.** Wrap only the operation that needs it, so the
  proof stays small.
- **Missing `// SAFETY:` or `# Safety`.** Turn on
  `clippy::undocumented_unsafe_blocks` and `clippy::missing_safety_doc`.
- **`static mut`.** Almost always wrong; use atomics, `Mutex` or `OnceLock`.
- **Hand-written extern declarations for big C APIs.** Use `libc` or bindgen.
- **The databroker gets this right by having none.** Everything it needs
  exists as safe APIs. Its one "unsafe" decision is a *dependency* choice:
  the optional jemalloc allocator (`databroker/src/main.rs:14-16`).
  Reviewing a crate like that means checking that its unsafe code is small,
  documented and tested under Miri.

## In the databroker and in the wild

- `databroker/src/main.rs:14-16`: `#[global_allocator]` switches to
  jemalloc, a C library, through `unsafe impl GlobalAlloc` inside the
  `jemallocator` crate.
- `databroker/Cargo.toml`: the `tls` feature pulls in `ring`, where C and
  assembly are wrapped in a safe Rust API.
- `bytes-1.12.1/src/bytes_mut.rs:328` `BytesMut::split_off`: a real-world
  `split_at_mut`. Read the `// SAFETY:` comments at `:336` and `:341`,
  which justify disjoint handles into one buffer. tonic uses `bytes` for
  every gRPC message the databroker sends.
- `tokio-1.53.1/src/sync/mpsc/block.rs:47` and `:208`: `MaybeUninit` slots
  in the channel the databroker uses for provider streams.
- `smallvec-1.16.2/src/lib.rs:636`: `union SmallVecData`, inline-or-heap
  storage using the fifth unsafe superpower (unions).
- std itself (`rustup component add rust-src`, then
  `$(rustc --print sysroot)/lib/rustlib/src/rust/library/core/src/slice/mod.rs`,
  `split_at_mut` at line 1986 for Rust 1.96): compare it with your e02.

(Registry paths are under `~/.cargo/registry/src/index.crates.io-*/`.)

## Exercises

| Exercise | Goal |
|---|---|
| c26_e01_raw_pointers | Minimal `unsafe` blocks with SAFETY comments; bounds-check before offsetting |
| c26_e02_split_at_mut | Write `split_at_mut` from raw parts; see why the `assert!` makes it sound |
| c26_e03_safe_abstraction_invariants | A UTF-8 path buffer whose safe methods protect an unsafe read |
| c26_e04_maybeuninit_and_layout | Build `[T; N]` element by element, panic-safe, with a drop guard |
| c26_e05_ffi_call_c | Safe wrappers over glibc `strlen`, `abs`, `qsort` |
| c26_e06_ffi_expose_rust | A C API over a Rust signal store: handles, `repr(C)`, callbacks, no unwinding into C |
| c26_e07_ub_catalogue | Name the UB in seven snippets and rewrite each in safe Rust |

## Further reading

- The Rust Book, ch. 20.1 "Unsafe Rust": <https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html>
- The Rustonomicon (the whole of it, eventually): <https://doc.rust-lang.org/nomicon/>
- Reference, "Behavior considered undefined": <https://doc.rust-lang.org/reference/behavior-considered-undefined.html>
- `std::ffi` module docs, `CStr` and `CString`: <https://doc.rust-lang.org/std/ffi/>
- Miri: <https://github.com/rust-lang/miri>
- *Rust for Rustaceans*, ch. 9 "Unsafe Code" and ch. 11 "Foreign Function Interfaces"
- The Embedded Rust Book: <https://docs.rust-embedded.org/book/>

## Check yourself

1. Name the five things `unsafe` allows.
2. Why is `split_at_mut` sound with the `assert!(mid <= len)` and unsound
   without it, even though the unsafe block is identical?
3. Why must the fields of a type with a safety invariant be private?
4. What goes wrong with `CString::new(path).unwrap().as_ptr()`?
5. What happens if a panic unwinds out of an `extern "C"` function, and
   what are two ways to prevent it?
6. Why must memory from `Box::into_raw` be freed with `Box::from_raw` and
   not C's `free`?
7. Which tool finds aliasing violations, and which do you use for code
   that calls C?

<details><summary>Answers</summary>

1. Dereference raw pointers, call unsafe functions (including FFI), access
   `static mut`, implement unsafe traits, and read union fields.
2. Soundness is about every *safe caller*. Without the check, safe code can
   pass `mid > len` and get slices outside the allocation. The unsafe block's
   proof depends on the precondition the safe code establishes.
3. Otherwise any safe code could break the invariant, and the unsafe code
   relying on it would cause UB without any `unsafe` at the site of the bug.
4. The temporary `CString` is dropped at the end of the statement, so the
   pointer dangles immediately (use-after-free).
5. The process aborts (Rust 1.81+). Catch the panic with `catch_unwind`
   and return an error code, or declare the function `extern "C-unwind"` if
   unwinding across the boundary is intended.
6. Rust's global allocator may not be malloc (for example jemalloc), and
   `Box::from_raw` also runs `T`'s destructor.
7. Miri finds aliasing violations. For FFI, use sanitizers
   (`-Zsanitizer=address`) or valgrind on Linux.
</details>
