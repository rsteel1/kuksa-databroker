# 01 — Basics: values, types, functions, control flow

**Why this matters for the databroker.** The databroker is, at heart, a big
table of vehicle signals (`Vehicle.Speed`, `Vehicle.Cabin.Door.Row1.Left.IsOpen`, …)
whose values are numbers, booleans and strings. Before we can talk about
ownership, async or gRPC, you need to be fluent with the small stuff: how
Rust stores a number, why `200 as i8` is `-56`, why a missing semicolon is
a type error, and why a variable can't change unless you say so. The
databroker's validation code (`Entry::validate_value` in
`databroker/src/broker.rs`) is almost entirely made of these basics.

> How to use this guide: read a section, then do the matching exercise
> (`cargo rl` shows you the next one). Every exercise names its section in
> the `GUIDE:` line at the top of the file.

---

## Variables and mutability

```rust
let speed = 120;        // an immutable binding
let mut count = 0;      // a mutable binding
count += 1;             // fine
// speed = 130;         // error[E0384]: cannot assign twice to immutable variable
```

`let` creates a *binding*: a name for a value. By default it is
**immutable** — once set it never changes. You opt into change with `mut`.

Why would a language make you type *more* to change a variable? Because
most variables never need to change, and knowing that a value *can't*
change is enormously useful when you read code. In a 5,000-line file like
`broker.rs` you can look at any `let` without `mut` and know its value is
fixed for its whole life. When you see `let mut`, it's a signal: "watch
this one, it evolves." For example `broker.rs:1533`:

```rust
let mut errors = Vec::new();   // grows as updates fail
```

Constants are different from immutable variables: they're computed at
compile time, must have a type annotation, and are written in
`SCREAMING_SNAKE_CASE`:

```rust
const MAX_SUBSCRIBE_BUFFER_SIZE: usize = 1000;   // broker.rs:38
```

**Type inference.** You rarely have to write types on `let`; the compiler
works them out from how the value is used. You *can* annotate:
`let count: u32 = 0;`. Function signatures, however, must always be fully
typed — they are the contract between pieces of code.

## Scalar types and conversions

| Type family | Types | Notes |
|---|---|---|
| signed integers | `i8 i16 i32 i64 i128 isize` | `i8` = −128..=127 |
| unsigned integers | `u8 u16 u32 u64 u128 usize` | `u8` = 0..=255 |
| floats | `f32 f64` | IEEE-754, has `NaN` |
| boolean | `bool` | `true` / `false` |
| character | `char` | a Unicode scalar value, 4 bytes |

`usize` is "an unsigned integer as wide as a pointer" (64 bits on the dev container and any other 64-bit machine).
Lengths and indexes are always `usize`: `vec.len()` returns one.

VSS (the Vehicle Signal Specification) has exactly these types — `int8`,
`uint16`, `float`, `double`, `boolean`, `string` and arrays of them — and the
databroker's `DataType` enum (`types.rs:17`) mirrors them one-to-one.

### Overflow

What is `250u8 + 10`? It doesn't fit in a `u8`. Rust's answer:

- in **debug** builds (what `cargo test` uses) it **panics**:
  `attempt to add with overflow`;
- in **release** builds it wraps around (to 4), unless you opt into checks.

Either way, overflow is treated as a bug. If you *want* specific behaviour,
say so: `a.checked_add(b)` (returns `None` on overflow), `a.wrapping_add(b)`,
`a.saturating_add(b)`.

### Conversions: `From`, `TryFrom`, `as`

There is no implicit conversion between numeric types in Rust — not even
from `u8` to `u32`. You must convert explicitly, and you choose *how*:

```rust
let small: u8 = 200;
let wide: u32 = u32::from(small);        // lossless, can't fail
let back: Option<u8> = u8::try_from(300u32).ok();  // None: doesn't fit
let chopped: i8 = 200i32 as i8;          // -56 !!  `as` silently truncates
```

- `T::from(x)` exists only when every value of the source fits in `T`.
  If it compiles, it's safe.
- `T::try_from(x)` returns a `Result` — `Ok(value)` or `Err(..)` — for
  conversions that might not fit. `.ok()` turns that into an `Option`
  (`Some`/`None`), which you'll meet properly in chapter 04.
- `x as T` never fails and never complains: it truncates, wraps, or rounds.
  Useful for floats (`len as f64`), dangerous for narrowing integers.

The databroker stores small integers (`int8`, `uint16`…) inside wider
variants and validates with `try_from` on every write — `broker.rs:552`:

```rust
DataType::Int8 => match value {
    DataValue::Int32(value) => match i8::try_from(*value) {
        Ok(_) => Ok(()),
        Err(_) => Err(UpdateError::OutOfBoundsType),
    },
    ...
```

That's the difference between "a provider sent 200 for an int8 signal and
we rejected it" and "the value silently became −56 and a dashboard showed
nonsense."

## Functions, statements and expressions

```rust
fn celsius_to_fahrenheit(celsius: f64) -> f64 {
    celsius * 9.0 / 5.0 + 32.0
}
```

- Parameters always have types; the return type follows `->`.
- A function with no `->` returns `()` ("unit"), Rust's "no meaningful value".
- The **last expression** of the body is the return value. `return x;`
  exists for returning early.

The key idea: Rust is an **expression language**. Blocks, `if`, `match` and
`loop` all produce values. A **statement** is something that doesn't: a
`let`, or any expression with `;` after it. So:

```rust
fn f() -> f64 { 1.0 }    // body evaluates to 1.0      ✅
fn g() -> f64 { 1.0; }   // body evaluates to ()       ❌ E0308: expected f64, found ()
```

That trailing `;` is the most common first error for Rust beginners. The
compiler message even suggests "remove this semicolon".

The databroker is full of one-expression functions: `types.rs:496-501`

```rust
pub fn new(id: i32) -> Self {
    Self { id }
}
pub fn id(&self) -> i32 {
    self.id
}
```

## Control flow

**`if` is an expression**, so it can produce a value:

```rust
let gear = if kmh <= 15 { 1 } else if kmh <= 30 { 2 } else { 3 };
```

Both branches must have the same type. There's no ternary `a ? b : c` —
`if` already does that job. The condition must be a `bool`; Rust never
treats `0` or `""` as false.

The databroker uses `if let` (chapter 04) as an expression to compute a
channel size — `broker.rs:1611`:

```rust
let channel_capacity = if let Some(cap) = buffer_size {
    ...
    cap + 1
} else {
    1
};
```

**Loops.**

```rust
for reading in &readings { ... }    // iterate a collection
for i in 0..10 { ... }              // a range: 0,1,...,9   (0..=10 includes 10)
while budget > 0 { ... }            // loop while a condition holds
loop { ... }                         // forever, until `break`
```

`break` leaves a loop; `continue` skips to the next iteration. `loop` is
special: `break value` makes the whole loop evaluate to `value`:

```rust
let first_power = loop {
    if candidate >= n { break candidate; }
    candidate *= 2;
};
```

The databroker's background housekeeping task is a `loop` that never
breaks — it wakes up once a second, cleans up dead subscriptions, and
sleeps again (`broker.rs:2372`).

## Tuples, arrays and shadowing

A **tuple** groups a fixed number of values of possibly different types:

```rust
let pair: (i32, i32) = (-7, 12);
let (lo, hi) = pair;          // destructuring
let lo_again = pair.0;        // positional access
```

Functions return several values by returning a tuple:
`fn min_max(r: [i32; 5]) -> (i32, i32)`.

An **array** `[T; N]` has a length fixed at compile time and lives inline
(no heap). For growable lists you'll use `Vec<T>` (chapters 02 and 05).
Indexing out of bounds panics — Rust never reads past the end of memory.

**Shadowing** re-declares a name:

```rust
let port = input.trim();              // &str
let port: u16 = port.parse().expect("valid port");   // now a u16
```

This is not mutation: it's a brand-new variable that hides the old one.
It can change the type — which `mut` can never do. Use it for "the same
thing, in a new form": text → trimmed text → number. The databroker parses
its `--port` flag into a `u16` through clap (`main.rs:287`); under the hood
that's the same `parse` idea.

---

## Mental model

| Concept | Picture it as |
|---|---|
| `let x = …` | a label stuck on a sealed box |
| `let mut x = …` | a label on a box you're allowed to reopen |
| shadowing | peeling the old label off and sticking it on a new box |
| integer types | boxes of a fixed size — `u8` is a box that only holds 0..=255 |
| `as` | forcing something into a box: whatever doesn't fit is cut off |
| `try_from` | asking politely: "does it fit?" — you get Yes(value) or No |
| expression | a question that produces an answer |
| `;` | "I don't care about the answer" |

**What the compiler guarantees here:** no variable changes unless declared
`mut`; no silent numeric conversion; no reading uninitialised variables;
integer overflow is caught in debug builds; array access is bounds-checked.

## Common misconceptions

| Wrong model | Correct model | Error |
|---|---|---|
| "Variables are mutable, `const` is for constants" | Bindings are immutable by default; `mut` opts in | E0384 |
| "`u8` will just grow if needed" | Types have fixed ranges; overflow panics in debug | runtime panic |
| "`as` converts safely" | `as` truncates/wraps silently; use `try_from` to check | (none — that's the danger) |
| "`return` is how functions return" | The last expression is the result; `;` makes it `()` | E0308 |
| "I can assign a number to my string variable" | A variable's type is fixed; shadow instead | E0308 |
| "`0` is false" | Conditions must be `bool` | E0308 |

Run `rustc --explain E0384` (or any code) for a long explanation with
examples — it's one of the best learning tools Rust has.

## Smells and idioms

- **`let mut` when nothing changes.** The compiler warns ("variable does
  not need to be mutable"); listen to it.
- **`as` for narrowing integers.** Prefer `try_from`. Keep `as` for
  `usize → f64` style conversions where rounding is acceptable and obvious.
- **Manual range checks.** `x >= 1 && x <= 65535` → `(1..=65535).contains(&x)`
  (clippy suggests this).
- **`x = x + 1`** → `x += 1` (clippy: `assign_op_pattern`).
- **Invented names** like `port_str`, `port_num` → shadowing.
- **`unwrap()` on parsing user input.** Fine in tests; in real code handle
  the error (chapter 07). In this chapter `expect("why it can't fail")` is
  used when a test guarantees valid input.

## In the databroker

1. `databroker/src/broker.rs:38-39` — two `const`s. Note the type
   annotations are mandatory and `Duration::from_secs(1)` is allowed
   because it's a `const fn`.
2. `databroker/src/broker.rs:517-560` — `Entry::validate_value`. Read the
   `DataType::Int8` arm at line 552: `try_from` used for range checking.
   Scroll on and notice the same pattern repeated for every integer width.
3. `databroker/src/types.rs:137-142` — `greater_than` widens an `i32` to an
   `i64` with `i64::from(*value)` before comparing it to an `i64`. Lossless
   widening, no `as`.
4. `databroker/src/broker.rs:1611-1620` — `if let … else` used as an
   expression to compute `channel_capacity`.
5. `databroker/src/broker.rs:2372` — the housekeeping `loop`.
6. `databroker/src/main.rs:287` — the port is parsed as a `u16` (0..=65535),
   so an out-of-range port is rejected at startup by the type itself.

## Exercises

| Exercise | Goal |
|---|---|
| `c01_e01_variables` | Fix an immutable counter with `mut` |
| `c01_e02_types_and_casts` | Avoid `u8` overflow; replace a lossy `as` with `try_from` |
| `c01_e03_functions` | The trailing-semicolon bug; write small functions |
| `c01_e04_control_flow` | `if` as an expression, `for`/`break`, `loop` with a value, `while` |
| `c01_e05_tuples_arrays_shadowing` | Shadowing instead of `mut`; return tuples; destructure |

## Further reading

- The Rust Book, ch. 3 "Common Programming Concepts" (3.1 Variables and
  Mutability, 3.2 Data Types, 3.3 Functions, 3.5 Control Flow) —
  <https://doc.rust-lang.org/book/ch03-00-common-programming-concepts.html>
- Rust by Example: "Primitives", "Types → Casting", "Flow of Control" —
  <https://doc.rust-lang.org/rust-by-example/>
- `std` docs for `i8::try_from`, `u32::checked_add`:
  <https://doc.rust-lang.org/std/primitive.i8.html>

## Check yourself

1. What does `let x = 5; x = 6;` produce, and how do you fix it two different ways?
2. What is `300i32 as u8`? What is `u8::try_from(300i32)`?
3. Why does `fn f() -> i32 { 5; }` fail to compile?
4. What value does `let v = loop { break 7; };` bind?
5. Why does the databroker use `try_from` when validating an `int8` write?
6. What's the difference between shadowing and `mut`?

<details><summary>Answers</summary>

1. E0384. Either `let mut x = 5; x = 6;` or shadow: `let x = 5; let x = 6;`.
2. `44` (300 − 256: the high bits are dropped). `Err(TryFromIntError)` — it doesn't fit.
3. `5;` is a statement, so the body evaluates to `()`, not `i32` (E0308).
4. `7`.
5. Because the value arrives as an `i32`, and a value outside −128..=127
   must be *rejected* (`OutOfBoundsType`), not silently truncated.
6. Shadowing creates a new variable (may change type, old one is hidden);
   `mut` lets the same variable change value but never type.

</details>
