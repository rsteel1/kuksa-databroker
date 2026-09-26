# 06 — Strings, text and formatting

**Why this matters for the databroker.** Almost every request carries text.
Signal paths like `Vehicle.Cabin.Door.Row1.DriverSide.IsOpen`, glob patterns
like `Vehicle.*.IsOpen`, units, descriptions, JWT scopes, and the values
people type into the CLI (`publish Vehicle.Speed 42`) all arrive as strings.
The broker splits them, validates them, parses them into numbers, and
formats them back out for logs and error messages. Rust's string types look
fussy at first, but each rule prevents a real bug: text that works in every
ASCII test and then panics on "°C" in a car.

This chapter comes before traits and error handling, so a few things appear
here in a simple form and are explained properly later. `Result` gets a
short introduction here and full coverage in chapter 07. `impl ... for` gets
a two-line explanation here and full coverage in chapter 09.

## UTF-8: bytes, chars and why s[0] doesn't compile

A `String` owns a buffer of **UTF-8 bytes** on the heap. A `&str` is a
borrowed view of some UTF-8 bytes: a pointer plus a length. The bytes are
always valid UTF-8, and the compiler and standard library guarantee it.

UTF-8 is a *variable-width* encoding:

| Text | Characters | Bytes |
|---|---|---|
| `"Speed"` | 5 | 5 (ASCII is 1 byte per char) |
| `"Größe"` | 5 | 7 (`ö` and `ß` are 2 bytes each) |
| `"°C"` | 2 | 3 |
| `"🚗"` | 1 | 4 |

That's why:

```rust
let s = "Größe";
s.len();              // 7: BYTES, O(1)
s.chars().count();    // 5: characters, O(n) (must decode)
// s[0]               // error[E0277]: the type `str` cannot be indexed by `{integer}`
s.chars().next();     // Some('G')
&s[0..2];             // "Gr": byte range, fine here
// &s[0..3]           // PANIC: byte 3 is inside 'ö'
```

Rust refuses `s[0]` because it would be either slow (decoding from the
start) or wrong (returning a byte and calling it a character). Instead you
choose explicitly:

| You want | Use |
|---|---|
| bytes | `s.as_bytes()`, `s.bytes()` |
| characters | `s.chars()` |
| characters *and* where they start | `s.char_indices()` → `(byte_offset, char)` |
| a safe cut point | a byte offset from `char_indices`, or `s.is_char_boundary(i)` |

A `char` is a *Unicode scalar value* (4 bytes, any code point except
surrogates). It is not always what a human calls a "letter": an emoji flag
or `é` written as `e` + a combining accent is several `char`s. For
user-visible text editing you'd need a grapheme-segmentation crate, but for
paths and protocol text, `char` is the right unit.

Most real code never indexes at all. It uses methods that work on whole
UTF-8 strings: `split`, `split_once`, `trim`, `starts_with`, `strip_prefix`,
`strip_suffix`, `contains`, `replace`, `find` (which returns a *byte* offset
that is always on a boundary).

## Building strings

```rust
let mut s = String::with_capacity(64); // one allocation up front
s.push_str("Vehicle");
s.push('.');
s += "Speed";                          // same as push_str
let a = format!("{s} = {}", 42);       // new String, borrows its inputs
let joined = ["Vehicle", "Cabin"].join(".");
```

Two things to know:

1. **`+` moves its left operand.** `a + &b` calls `Add::add(a, &b)`. It
   takes `a` by value, appends to its buffer, and returns it. That's
   efficient (no new allocation), but `a` is gone afterwards (E0382). Use
   `format!` when you need to keep the inputs; it only borrows them.
2. **`write!` / `writeln!` can target a `String`.** With
   `use std::fmt::Write;` in scope you can append formatted text directly,
   with no temporary `String` per line:

   ```rust
   use std::fmt::Write;
   let mut out = String::new();
   for (path, v) in rows {
       writeln!(out, "{path}={v:.1}").expect("writing to a String cannot fail");
   }
   ```

   (`std::fmt::Write` is a *trait*, a set of methods a type promises to
   provide. Chapter 09 covers traits. For now, the `use` line "switches on"
   `write!` for `String`s.)

## Parsing text: parse and FromStr

`str::parse` turns text into any type that implements the standard
`FromStr` interface. Numbers, `bool`, `char`, IP addresses and many more
already do:

```rust
let n: u8 = "42".parse().expect("valid");      // type from the annotation
let x = "1.5".parse::<f64>();                  // or with a "turbofish"
let bad = "300".parse::<u8>();                 // Err: number too large
```

Parsing can fail, so `parse` returns a `Result<T, E>`. It works like
`Option`, except the failure case carries a reason: `Ok(value)` or
`Err(error)`. You handle it with `match`, or turn it into an `Option` with
`.ok()`. Chapter 07 is all about `Result`.

To make *your* type parseable, implement `FromStr`:

```rust
use std::str::FromStr;

impl FromStr for Update {
    type Err = ParseUpdateError;             // what a failure looks like
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (path, value) = match s.split_once('=') {
            Some(parts) => parts,
            None => return Err(ParseUpdateError::MissingEquals),
        };
        // ...
    }
}

let u: Update = "Vehicle.Speed=42".parse()?;   // now this works
```

`impl FromStr for Update` means "`Update` fills in the standard *parse from
a string* interface". `type Err` names its error type. Both ideas return
with full detail in chapters 09 and 14.

## Formatting: Display, Debug and format specs

`format!`, `println!`, `write!` and friends share one mini-language:

| Spec | Meaning | `42.5` / `"Vehicle"` becomes |
|---|---|---|
| `{}` | `Display`: for people | `42.5` |
| `{:?}` | `Debug`: for programmers | `42.5`, `"Vehicle"` (quoted) |
| `{:#?}` | pretty `Debug` (multi-line) | fields on separate lines |
| `{:.2}` | precision | `42.50` |
| `{:8}` / `{:>8}` / `{:<8}` / `{:^8}` | width + alignment | `    42.5` |
| `{:08.2}` | zero-padded | `00042.50` |
| `{:x}` / `{:#x}` / `{:#06x}` | hex, with `0x`, padded | `ff`, `0xff`, `0x00ff` |
| `{name}` | inline variable (Rust 2021) | `format!("{path}")` |

- **`Debug` can be derived** (`#[derive(Debug)]`), and every public type
  should derive it. The output shows the structure, including quotes and
  field names.
- **`Display` is never derived.** Only you know how your type should look
  to a person. You implement it by writing to the `Formatter`:

  ```rust
  impl fmt::Display for Reading {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
          write!(f, "{} = {:.2}", self.path, self.value)
      }
  }
  ```

  Once `Display` is implemented you get `.to_string()` for free.

- **Respecting the caller's width.** If a caller writes `{:>6}` and your
  `fmt` does `write!(f, "{}", self.0)`, the width is silently ignored,
  because it was applied to the *outer* placeholder, not your inner `{}`.
  For text, `f.pad(&text)` applies the width, fill, alignment and precision
  for you. For numbers, forward the formatter to the inner value's `fmt`.

## Advanced patterns

Chapter 04 introduced `match`. Patterns can do a lot more:

```rust
match parts {                                   // parts: &[&str]
    [] => "empty",
    ["Vehicle"] => "root",
    ["Vehicle", .., "IsOpen"] => "status",      // first and last element
    ["Vehicle", rest @ ..] if rest.len() > 3 => "deep",  // bind the tail + guard
    ["Vehicle", ..] => "vehicle",
    _ => "foreign",
}

match kmh {
    n @ 0..=30 => format!("slow ({n})"),        // `@`: bind AND test
    n => format!("other ({n})"),
}

matches!(value, Value::Int(_) | Value::Float(_)) // a pattern as a bool
```

**Nested destructuring** looks straight through layers:

```rust
if let Update { target: Some(Some(Value::Int(t))), .. } = &update { /* t: &i64 */ }
```

**Binding modes ("match ergonomics").** When you match a *reference*
against a non-reference pattern, Rust automatically binds the inner parts
by reference:

```rust
for value in values.iter_mut() {        // value: &mut Value
    if let Value::Int(n) = value {      // n: &mut i64 (not i64)
        *n += 1;
    }
}
```

Older code writes this explicitly with `ref` / `ref mut`
(`Value::Int(ref mut n)`). You'll still see that in the wild, and
occasionally you need it to borrow instead of move out of an owned value.

Patterns are everywhere in real code:
`let (path, value) = s.split_once('=')?;`, `for (id, entry) in &map`,
closure parameters `|(k, v)|`, and function parameters
`fn f(&(a, b): &(i32, i32))`.

## Cow: borrow when you can, own when you must

Many text functions *usually* return their input unchanged and
*occasionally* need to modify it. Returning `String` forces an allocation
every time. Returning `&str` can't express the modified case.
`std::borrow::Cow` ("clone on write") holds either:

```rust
enum Cow<'a, str> {          // simplified
    Borrowed(&'a str),       // no allocation
    Owned(String),           // allocated because something changed
}

fn to_glob_pattern(input: &str) -> Cow<'_, str> {
    if !input.contains('.') && input.contains('*') {
        return Cow::Borrowed(input);         // common case: free
    }
    let mut out = input.replace('.', "/");
    if !input.contains('*') { out.push_str("/**"); }
    Cow::Owned(out)
}
```

Callers use the result like a `&str` (it *derefs* to `str`, as chapter 14
explains), or call `.into_owned()` if they need a `String`. The standard
library does the same: `String::from_utf8_lossy` returns `Cow<str>` because
valid input doesn't need copying. The `'_` says "this may borrow from the
input", which chapter 11 explains.

## Mental model

| Idea | Analogy |
|---|---|
| `String` | A notebook you own: you can write in it and it grows |
| `&str` | A photocopy of some pages: read-only, the original must stay around |
| UTF-8 | A language where some words take 1 syllable and others 4, so you can't find word 10 without reading from the start |
| `char_indices` | Reading aloud while noting the page position of each word |
| `Display` vs `Debug` | The shop window vs the inventory sheet |
| `Cow` | Hand over the original document unless you had to annotate it, in which case hand over your annotated copy |

**What the compiler guarantees:** a `&str`/`String` always holds valid
UTF-8, and you can never obtain a `&str` that splits a character. Operations
that could break this either don't exist (`s[0]`) or panic loudly
(`&s[0..3]` inside a char), never silently corrupt text.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "`len()` is the number of characters." | `len()` is bytes. Use `chars().count()`, and know it is O(n). |
| "I can index a string like an array." | E0277. Use `chars()`, `char_indices()` or string methods. |
| "Byte-range slicing is always safe." | It panics if a boundary falls inside a character. Take offsets from `find` or `char_indices`. |
| "`a + b` copies both strings." | It *moves* `a` (E0382 if you use it later) and borrows `b`. |
| "`#[derive(Display)]` exists." | Only `Debug` derives. `Display` is always hand-written (or comes from a helper crate like `thiserror`). |
| "Returning `Cow` is premature optimisation." | In hot paths (every request parses paths) avoiding an allocation per call is real. Elsewhere, returning `String` is fine. |
| "`ref` is needed to bind by reference." | Match ergonomics does it automatically when you match on a reference. |

## Smells and idioms

- **Take `&str`, not `&String`.** `&String` gives nothing extra and rejects
  string literals and sub-slices (clippy: `ptr_arg`). Take `impl AsRef<str>`
  or `impl Into<String>` when you want to accept several kinds (chapter 09).
- **Don't build strings in a loop with `+` or `format!` per piece.**
  Pre-size with `String::with_capacity` and use `push_str` / `write!`.
- **Don't `to_string()` just to compare** (`s.to_string() == "x"`). Compare
  `&str`s directly.
- **Prefer `split_once`, `strip_prefix` and `strip_suffix`** over manual
  `find` + slicing.
- **Derive `Debug` on every public type.** Implement `Display` only when
  there's one obvious human representation.
- **In the databroker:** `glob.rs` builds its glob pattern with
  `String::new()`, then `push_str` of another freshly allocated `String`.
  That's two allocations where one (or zero, with `Cow`) would do. It's
  harmless at startup but worth knowing in per-request code. `types.rs`'s
  `Display for DataValue` falls back to `{values:?}` for arrays, which
  prints with Debug quoting (`["a", "b"]`). That's a pragmatic choice rather
  than a polished human format.

## In the databroker and in the wild

- `databroker/src/types.rs:44`, `impl fmt::Display for DataType`: a plain
  `match` writing a fixed name per variant.
- `databroker/src/types.rs:112`, `impl fmt::Display for DataValue`. Note
  `DataValue::BoolArray(values) => write!(f, "{values:?}")` at line 124,
  where Display delegates to Debug for arrays.
- `databroker-proto/src/lib.rs:49`, `impl FromStr for DataType` with
  `type Err = ParsingError` (line 50). It lowercases the input and matches
  `"int8[]"` etc. Line 82 onward, `Value::new`, parses the text value with
  `value.parse::<i8>()` and friends and maps each error into a
  `ParsingError`. This is how the CLI turns `publish Vehicle.Speed 42` into
  a typed protobuf value.
- `databroker-proto/src/lib.rs:41`, `impl Display for ParsingError`, which
  forwards to the inner `String`'s formatting (`self.message.fmt(f)`) so the
  caller's width and precision are honoured, the same idea as `f.pad`.
- `databroker/src/glob.rs:72`, `Matcher::to_glob_string`, the
  `replace('.', "/")` + `"/**"` logic you rewrote with `Cow` in `c06_e06`.
- `databroker/src/glob.rs:202`, `is_valid_path`, which validates with a
  regex over the whole `&str` rather than indexing (chapter 29 goes deep).
- `databroker/src/authorization/jwt/decoder.rs:110`,
  `(KeyKind::Rsa, a @ (Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512)) => a`,
  an `@` binding over an or-pattern inside a tuple pattern.
- `databroker/src/broker.rs:1216`, `(Some(_), Err(PermissionError::Denied)) => ...`,
  tuple patterns combining an `Option` and a `Result` in one `match`.
- `databroker/src/viss/server.rs:104`, `reason: Cow::from("Unsupported websocket subprotocol")`,
  a borrowed `'static` string inside a `Cow` field that could also hold an
  owned one.
- `databroker/src/main.rs:244`, `format!(concat!(...), ...)` builds the
  multi-line `--version` text. `concat!` joins string *literals* at compile time.
- In std (rust-src component,
  `~/.rustup/toolchains/stable-*/lib/rustlib/src/rust/library/`):
  `alloc/src/string.rs` `pub fn from_utf8_lossy` returns `Cow<'_, str>`,
  and `core/src/fmt/mod.rs` `pub fn pad` is exactly the width-handling you
  used in `c06_e04`.

## Exercises

| Exercise | Goal |
|---|---|
| `c06_e01_utf8_bytes_chars` | Bytes vs chars, and truncating without splitting a character |
| `c06_e02_string_building` | `format!` vs `+`, `join`, `write!` into a pre-sized `String` |
| `c06_e03_parse_fromstr` | `parse::<T>()` and implementing `FromStr` with precise errors |
| `c06_e04_formatting_traits` | Derive `Debug`, hand-write `Display`, format specs, `Formatter::pad` |
| `c06_e05_advanced_patterns` | Slice patterns, `@`, guards, nested destructuring, match ergonomics |
| `c06_e06_cow_str` | Return borrowed text when nothing changes, owned when it must (the databroker's glob conversion) |

## Further reading

- The Book, ch. 8.2 "Storing UTF-8 Encoded Text with Strings":
  <https://doc.rust-lang.org/book/ch08-02-strings.html>
- The Book, ch. 19 "Patterns and Matching" (all pattern syntax):
  <https://doc.rust-lang.org/book/ch19-00-patterns.html>
- `std::fmt` module docs (the complete format-spec grammar):
  <https://doc.rust-lang.org/std/fmt/>
- `std::borrow::Cow`: <https://doc.rust-lang.org/std/borrow/enum.Cow.html>
- Rust by Example, "Strings" and "Formatted print".

## Check yourself

1. Why does `"Größe".len()` return 7?
2. What happens at runtime with `&"Größe"[0..3]`, and how do you find a safe cut point?
3. After `let c = a + &b;`, can you still use `a`? `b`?
4. Why can `Debug` be derived but `Display` can't?
5. Your `Display` impl ignores `{:>10}`. Why, and how do you fix it for text?
6. What does `n @ 1..=9` do in a pattern?
7. When is returning `Cow<str>` better than returning `String`?

<details><summary>Answers</summary>

1. `len()` counts UTF-8 bytes. `ö` and `ß` take 2 bytes each: 3 + 2 + 2 = 7.
2. It panics, because byte 3 falls inside `ö`. Take offsets from
   `char_indices()`, `find()`, or check `is_char_boundary`.
3. `a` is moved into the `+` (E0382 if used). `b` was only borrowed, so yes.
4. `Debug` has a mechanical answer (show the structure). `Display` needs a
   human decision about how the value should look.
5. The width applies to the outer placeholder, and your inner `write!("{}")`
   has none. Use `f.pad(&text)`.
6. It tests that the value is in 1..=9 and binds it to `n`.
7. When the common case needs no change, so you can hand back the input
   borrowed with no allocation, and only allocate when you must modify it.
</details>

## Where this goes next

Chapter 29 applies these string tools to VSS path validation and glob matching. Chapter 15 covers `Box<str>`/`Arc<str>`, and chapter 27 covers avoiding string allocations on hot paths.
