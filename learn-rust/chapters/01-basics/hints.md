# Chapter 01 hints

## c01_e01_variables
The compiler error is E0384: `cannot assign twice to immutable variable`.
Run `rustc --explain E0384`.

Bindings are immutable by default. One keyword, placed right after `let`,
makes a binding mutable: `let mut count = 0;`

## c01_e02_types_and_casts
The first failure is a *runtime* panic: in debug builds Rust checks integer
arithmetic and panics on overflow instead of silently wrapping.

`u8` holds 0..=255. 200 + 250 does not fit. Accumulate into a `u32`:
`let mut total: u32 = 0;` and add `u32::from(*reading)` (lossless widening).
Divide by `readings.len() as u32`, then turn the result back into `u8` with
`u8::try_from(average).expect("...")` (it can't fail — why?).

For `to_int8`: `200 as i8` is `-56` (the high bits are thrown away).
`i8::try_from(value)` returns a `Result`; `.ok()` turns it into an `Option`.

## c01_e03_functions
E0308 says the function body evaluated to `()`. Look at the last line of
`celsius_to_fahrenheit`: the trailing `;` turns the expression into a
statement. Remove it.

`is_valid_port`: `port > 0 && port <= 65535` works; clippy prefers
`(1..=65535).contains(&port)`.

`ms_to_seconds`: you can't divide a `u32` by a `f64`. Convert first:
`f64::from(ms) / 1000.0`.

## c01_e04_control_flow
`if` is an expression, so the whole `if / else if / else` chain can be the
function's final expression — no `return` needed.

For the sentinel: `for &value in values { if value == -1 { break; } ... }`.

`loop` can return a value: `break candidate;` ends the loop AND makes the
loop expression evaluate to `candidate`. Start at 1 and keep doubling.

For `halvings`, the parameter is already `mut budget`: loop
`while budget > 0 { budget /= 2; count += 1; }`.

## c01_e05_tuples_arrays_shadowing
E0308: `port` was declared as `&str` (the result of `trim()`), and a `&str`
variable can never hold a `u16`. A variable's type is fixed forever.

Shadowing creates a *new* variable that happens to reuse the name:
```rust
let port = input.trim();
let port: u16 = port.parse().expect("valid port");
```
No `mut` needed.

`min_max`: start both at `readings[0]` and walk the array with a `for`.
Return `(min, max)`.

`swap`: destructure with `let (name, id) = pair;` then return `(id, name)`.

## c01_e06_build_datapoint_stats
There's no bug to find here — the tests are the spec, and you write every
function body from scratch.

`clamp`: `i32` already has a `.clamp(min, max)` method; use it.

`scale_to_percent`: guard `raw_max <= 0` first and return `0`. Otherwise the
danger is `raw * 100` overflowing `i32` — widen both operands with
`i64::from(...)` before multiplying, divide, then `.clamp(0, 100)`, then
cast to `u8` at the very end (never before).

`count_out_of_range`: loop over `&readings` with `for &r in readings`, and
test `!(min..=max).contains(&r)`.

`average`: guard the empty case and return `0.0` (dividing by zero as a
float gives `NaN`, which the test doesn't want). Sum into an `i64`
accumulator (so many readings can't overflow `i32`), then divide by
`readings.len() as f64`.

## c01_e07_review_calibration
Read each function against its doc comment; the bug is where the code
doesn't do what the comment says.

Bug 1 (`calibrate`): `(mv - offset_mv) as i16` narrows to `i16` (range
±32767) *before* dividing. `50_000` doesn't fit, so it wraps to a negative
number. Drop the `as i16` — do the whole computation in `i32`.

Bug 2 (`running_total`): look closely at the loop body —
`let total = total + window[i];` declares a *new* `total` that shadows the
outer one and is immediately dropped at the end of that iteration. The
outer accumulator never changes. Use `total += window[i];` (an assignment,
not a new `let`) instead.

Bug 3 (`first_n_average`): `0..n - 1` is one short of `0..n`; for `n = 4`
it only visits indices 0..=2, skipping the fourth reading. Loop over
`&window[..n]` instead (or fix the range to `0..n`).
