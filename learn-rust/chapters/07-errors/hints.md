# Chapter 07 hints

## c07_e01_result_basics
`input.trim().parse::<u16>()` returns `Result<u16, ParseIntError>`. Match on it.
Match arms can have guards: `Ok(n) if n > 400 => Err(...)` must come *before*
the plain `Ok(n) => Ok(n)` arm. Negative numbers and 70000 both fail to parse
as `u16`, so they land in the `Err(_)` arm — build the message from the
original `input`, not the trimmed one.
`describe` takes `&Result<..>`; matching on a reference gives you references
(`Ok(speed)` binds `&u16`), which `format!` prints fine.

## c07_e02_question_mark
Read the error: `?` only works in a function returning `Result`/`Option`.
The tests compare against `Ok((...))` / `Err("...".to_string())`, so the
return type must be `Result<(String, i32), String>` — and the last line must
wrap the tuple in `Ok(...)`.
`split_once('=')` returns `Option<(&str, &str)>`. `.ok_or("missing '='")?`
turns `None` into an early `Err`. (`?` converts the `&str` error into the
function's `String` error type via `From<&str> for String`.)
For the empty path: `if path.is_empty() { return Err("empty path".to_string()); }`
works; the solution shows a one-expression variant with `bool::then_some`.

## c07_e03_custom_error_enum
`to_string()` comes from `Display`. Write
`impl fmt::Display for UpdateError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { ... } }`
and `match self` with one `write!(f, "...")` per variant. Struct-like variants
are matched as `UpdateError::NotFound { path } => ...`.
`impl std::error::Error for UpdateError {}` — empty body is fine: all its
methods have defaults, it only requires `Debug + Display`.
`validate`: check the prefix with `starts_with("Vehicle.")` first, then the
range with `(min..=max).contains(&value)` (an inclusive range).

## c07_e04_from_conversions
The compiler tells you exactly which conversions are missing:
`From<serde_json::Error>` and `From<ParseIntError>` for `VssError`.
```rust
impl From<serde_json::Error> for VssError {
    fn from(err: serde_json::Error) -> Self { VssError::Json(err.to_string()) }
}
```
Write the `ParseIntError` one the same way, mapping to `BadNumber`.
The `ok_or(VssError::MissingField("max"))?` line already produces a
`VssError`, so it needs nothing (`From<T> for T` always exists).

## c07_e05_thiserror
1. Derive it: `#[derive(Debug, thiserror::Error)]`.
2. Every variant needs a message: `#[error("signal already provided: {id}")]`
   (named fields are used by name).
3. For tuple variants refer to fields by position: `{1}` is the `String` in
   `InvalidId(ParseIntError, String)`.
4. Mark the inner error `#[source]` to expose it via `.source()`.
   `#[from]` does the same *and* generates `impl From<io::Error>`, which
   `.into()` in `send` needs. (`#[from]` requires the variant to contain only
   that one field, which is why `InvalidId` uses `#[source]` instead.)

## c07_e06_panic_vs_result
`port_from_env`: `value.and_then(|v| v.parse().ok())` gives `Option<u16>`
(parse failure and missing value both become `None`, and "99999" doesn't fit
in u16). Then `.expect("KUKSA_PORT must be a valid port")`.
`buffer_size`: `parse::<usize>()` with `.map_err(|_| format!(...))?`, then
check `> 1000`.
`parse_config`: split on `';'`, then `split_once('=')` and `match` on the
pair: `Some(("port", text)) => ...`. For the port use
`text.parse::<u16>().with_context(|| format!("invalid port `{text}`"))?`
(`Context` must be imported — it is). Use `let Some(port) = port else { bail!("missing port") };`
(let-else) and `bail!` for the other errors.

## c07_e07_build_update_pipeline
No bug to fix — `ParseError`'s shape and the tests are the spec; you
derive the trait and write the functions.

Derive: `#[derive(Debug, PartialEq, thiserror::Error)]`, with one
`#[error("...")]` above each variant, copied from its doc comment. Tuple
fields are `{0}`; named fields use their name, and `{field:?}` uses
`Debug` (so a `String` prints with quotes, matching the expected
messages exactly).

`parse_line`: trim first; empty means `Err(ParseError::Empty)`. Then
`split_once('=')` -> `ok_or_else(|| ParseError::MissingEquals(...))?`,
then `.trim().parse::<f64>()` -> `.map_err(|_| ParseError::BadNumber { .. })?`.

`parse_all`: loop over `input.lines()`, skip blank ones, and push
`parse_line(line)?` — the `?` is what makes it stop at the first error.

`parse_all_lenient`: same loop, but `match parse_line(line) { Ok(p) =>
oks.push(p), Err(e) => errs.push(e) }` instead of `?`, so nothing ever
stops the loop.

## c07_e08_review_batch_apply
Compare each function's doc comment against what it actually does.

Bug 1 (`apply_batch`): `parse_entry(i, raw)` passes the 0-based index
from `enumerate()`, but `parse_entry`'s own doc comment says it wants a
1-based line number. Pass `i + 1`.

Bug 2 (`apply_batch`): `if let Ok((path, value)) = parse_entry(...) { ... }`
does nothing when `parse_entry` returns `Err` — the loop just moves on to
the next line instead of aborting, exactly the "silently skipped"
behaviour the doc comment forbids. Match on the `Result` (or use `?`) so
the error actually propagates out of `apply_batch`.

Bug 3 (`first_error_message`): `Some("invalid entry".to_string())` is a
fixed string that never looks at the real error — the doc comment
promises the actual line and entry. Use the error's own `Display` (which
you get for free from `thiserror`): `Some(e.to_string())`.
