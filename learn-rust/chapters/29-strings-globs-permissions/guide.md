# 29 — Strings, globs & permissions

> **Builds on:** chapters 05 (collections), 06 (strings), 12 (LazyLock), 14 (builders and typestate).

**Why this matters for the databroker.** Every request names signals by
path — `Vehicle.Cabin.Door.Row1.DriverSide.IsOpen` — and every request is
checked against the caller's **permissions**, which are themselves lists of
path *patterns* taken from a JWT (`read:Vehicle.Cabin.* provide:Vehicle.Speed`).
So three small modules sit on the hot path of every call:
`databroker/src/glob.rs` (validating and matching paths),
`databroker/src/permissions.rs` (the `Permissions` type and its builder),
and `databroker/src/authorization/jwt/` (turning a token's scope string into
`Permissions`). This chapter covers the Rust you need for them: `&str`
handling, regexes, lazy statics, and the builder pattern.

---

## Working with VSS paths as &str

(Chapter 06 covered UTF-8, building, parsing and formatting strings in depth.
This section is a quick recap of the path-handling methods the databroker uses.)

Recall from chapter 02: `String` owns a growable UTF-8 buffer; `&str` is a
borrowed view into *some* UTF-8 bytes (part of a `String`, a literal, ...).
Path handling is mostly **looking at** strings, so functions take `&str`:

```rust
fn parent(path: &str) -> Option<&str> {
    path.rsplit_once('.').map(|(parent, _leaf)| parent)
}
```

The returned `&str` points *into the caller's string* — no allocation. The
elided signature is really `fn parent<'a>(path: &'a str) -> Option<&'a str>`:
"the result lives as long as the input". (Chapter 11 covers this.)

The methods you'll use constantly:

| Method | Returns | Example |
|---|---|---|
| `split('.')` | iterator of `&str` | `"A.B".split('.')` → `"A"`, `"B"` |
| `split_once(':')` / `rsplit_once('.')` | `Option<(&str, &str)>` | `"read:Vehicle"` → `("read", "Vehicle")` |
| `split_whitespace()` | iterator of `&str` | JWT scopes |
| `strip_prefix("Bearer ")` / `strip_suffix("/**")` | `Option<&str>` | the auth header, glob suffixes |
| `starts_with`, `ends_with`, `contains` | `bool` | |
| `replace('.', "/")` | **new `String`** | VSS path → glob path |
| `[&str]::join(".")` | **new `String`** | segments → path |
| `chars()` | iterator of `char` | per-character checks (Unicode-aware) |
| `trim()` | `&str` | `main.rs` trims VSS file names |

Only `replace`, `join`, `to_string`, `format!` allocate. Everything else is
slicing. `strip_prefix` returning `Option` is much safer than the index
arithmetic you'll find in `grpc/server.rs` (`header[7..]` after a
`starts_with("Bearer ")` check) — slicing a `&str` at a byte index that isn't
a character boundary **panics**.

**Unicode**: VSS paths may contain `ñ`, `ö`, `東京` — the databroker's tests
check that (`databroker/src/glob.rs` `test_is_valid_path`). `chars()`
iterates Unicode scalar values, so per-character checks work;
`bytes()` would see the individual UTF-8 bytes. `.len()` is in **bytes**.

## Validating paths: iterators vs regex

A path is valid when it's one or more non-empty segments separated by `.`,
with no `.`, `:`, `*` or whitespace inside a segment. Two equivalent ways:

```rust
// Iterator style: no dependencies, easy to read, easy to debug.
fn is_valid_path(path: &str) -> bool {
    path.split('.').all(|seg| {
        !seg.is_empty() && !seg.chars().any(|c| c == '.' || c == ':' || c == '*' || c.is_whitespace())
    })
}

// Regex style: compact, and the same pattern language as other tools.
static VALID_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^.:*\s]+(?:\.[^.:*\s]+)*$").expect("static regex is valid"));
```

The databroker uses the regex style (`databroker/src/glob.rs:181`) with
`(?x)` "verbose mode", which allows whitespace and `#` comments inside the
pattern — worth copying when a regex gets long.

### Lazy statics: compile once, use forever

Building a `Regex` parses and compiles the pattern — far too expensive to do
per request. The regex must be created once and shared by every thread. But
`static` values must be built at compile time, and `Regex::new` can't run at
compile time. The fix is lazy initialisation: build on first access,
thread-safely.

| Tool | Status |
|---|---|
| `lazy_static! { static ref X: T = ...; }` | The `lazy_static` crate. Used by the databroker (`glob.rs:146`, `permissions.rs:22`). Works, but it's a macro and an extra dependency. |
| `once_cell::sync::Lazy<T>` | Crate that std's version was modelled on. |
| `std::sync::LazyLock<T>` (Rust 1.80) | **Use this in new code.** Plain type, no macro. |
| `std::sync::OnceLock<T>` (Rust 1.70) | When the init needs runtime input: `CELL.get_or_init(|| ...)`. |

Either way, `VALID_PATH.is_match(..)` derefs to the inner `Regex`,
initialising it on the first call. `.expect("static regex is valid")` is the
right choice here: a bad *literal* regex is a programming bug, and the
message says why the panic "can't happen".

## VSS wildcards and the Matcher type

Clients may ask for patterns: `Vehicle.Cabin.*` (direct children),
`Vehicle.**.IsOpen` (any depth), or a plain branch `Vehicle.Cabin` (meaning
*that branch and everything below*). The full rules are in
`doc/wildcard_matching.md`.

The databroker delegates the actual matching to the `glob-match` crate,
which uses `/` separators like file paths. `Matcher` (`glob.rs:29`) is a
thin **validated wrapper**:

```rust
pub struct Matcher { glob_pattern: String }   // private field!

impl Matcher {
    pub fn new(glob_pattern: &str) -> Result<Self, MatchError> {  // glob.rs:34
        // "" or "\"\"" -> "**"; validate; translate once
    }
    pub fn is_match(&self, path: &str) -> bool { ... }             // glob.rs:56
}
```

Because the field is private, the *only* way to get a `Matcher` is
`Matcher::new`, which rejects bad patterns. Every `Matcher` in the program
is therefore valid — "**parse, don't validate**": check once at the
boundary, and let the type carry the proof.

The translation (`to_glob_string`, `glob.rs:72`): `.` → `/`, and if the
pattern has no `*` at all, append `/**` so `Vehicle.Cabin` covers its
children. Then a subtlety: in `glob-match`, `Vehicle/Cabin/**` does **not**
match `Vehicle/Cabin` itself. `is_match` handles this by retrying without
the suffix (`glob.rs:62`), so `Vehicle.Speed` still matches the signal
`Vehicle.Speed`.

Where it's used: `ListMetadata` in the v2 API
(`databroker/src/grpc/kuksa_val_v2/val.rs:785`) and the v1 `Get`/`Subscribe`
paths (`databroker/src/grpc/kuksa_val_v1/val.rs:96`). The broker precomputes
each entry's slash-separated `glob_path` at registration
(`databroker/src/broker.rs:1292`) so matching doesn't re-`replace` every
path on every request.

Two parallel matching engines exist: `Matcher` (glob-match) for request
patterns, and `to_regex_string` (`glob.rs:88`) → `RegexSet` for
*permissions* (`permissions.rs:295`). Historical reasons; worth knowing when
you debug "why does this pattern behave differently in a token than in a
request?".

## The builder pattern

`Permissions` has five parts (four path matchers + expiry). Its
constructor would need five arguments, several of them "nothing". Instead:

```rust
let permissions = Permissions::builder()
    .add_read_permission(Permission::Glob("Vehicle.Cabin.*".into()))
    .add_provide_permission(Permission::All)
    .expires_at(exp)
    .build()?;
```

Each method takes `mut self` **by value** and returns `Self`
(`permissions.rs:102`, `:107`):

```rust
pub fn add_read_permission(mut self, permission: Permission) -> Self {
    match permission {
        Permission::Nothing => {}
        Permission::All => self.read.extend_with(PathMatchBuilder::Everything),
        Permission::Glob(path) => self.read.extend_with_glob(path),
    };
    self
}
```

Why by value? Chains read naturally with no `&mut` juggling, and after
`build(self)` the builder has been *moved* — you can't accidentally keep
using a half-configured builder. The cost: in a loop you must reassign,
which is exactly what the JWT code does (`decoder.rs:134`):
`permissions = permissions.add_read_permission(...)`.

The alternative style — `fn x(&mut self) -> &mut Self` — lets you keep the
builder and build variations; `build(&self)` must then clone. You'll use it
in `c32_e04`.

Each scope is a small state machine, `PathMatchBuilder` (`permissions.rs:61`):

```text
Nothing ──glob──▶ Globs([..]) ──glob──▶ Globs([.., ..])
   │                  │
   └──────All─────────┴──────▶ Everything   (absorbing)
```

`build()` (`permissions.rs:151`, `:290`) is the single place that compiles
globs into a `RegexSet` and can fail. Validation lives in one spot; the
finished `Permissions` is immutable and cheap to check.

## From a JWT scope to an access decision

1. The interceptor (`databroker/src/grpc/server.rs`, chapter 30) takes the
   `authorization: Bearer <jwt>` header.
2. `Decoder::decode` (`authorization/jwt/decoder.rs:125`) verifies the
   signature and the `kuksa.val` audience (`decoder.rs:121`) and returns
   `Claims` (`decoder.rs:51`): `sub`, `iss`, `aud`, `iat`, `exp`, `scope`.
3. `parse_whitespace_separated` (`authorization/jwt/scope.rs:33`) splits
   `"read:Vehicle.Speed provide"` into `Scope { action, path: Option<String> }`.
4. `impl TryFrom<Claims> for Permissions` (`decoder.rs:134`) feeds each scope
   into the builder — no path means `Permission::All` — and sets
   `expires_at(UNIX_EPOCH + exp)` (`decoder.rs:178`).
5. Handlers call `can_read` / `can_write_datapoint` /
   `can_write_actuator_target` / `can_create` (`permissions.rs:167`–`215`).

The rules in step 5:

* **Expiry is checked first**, on every call (`is_expired`, `permissions.rs:228`).
  A long-lived subscription gets cut off when its token expires — the
  housekeeping task in `broker.rs` removes subscriptions whose permissions
  expired.
* **Read is implied** by actuate, provide and create (`permissions.rs:176`):
  if you may write a signal, you may read it.
* Writing a sensor value needs **provide**; setting an actuator target
  needs **actuate**; adding a brand-new entry to the broker's database needs
  **create** (`databroker/src/broker.rs:1275`).

`PermissionError` has two variants, `Denied` and `Expired`, because they map
to different gRPC codes: `PERMISSION_DENIED` vs `UNAUTHENTICATED`
("get a new token").

## Mental model

| Concept | Analogy |
|---|---|
| `&str` | A window onto someone else's text — you can read, not keep |
| `String` | Your own notebook page |
| `LazyLock<Regex>` | A machine built the first time someone needs it, then shared by everyone |
| Validated newtype (`Matcher`) | A stamped ticket: checked once at the gate, trusted afterwards |
| Consuming builder | A form passed hand to hand; each person fills a box and passes it on; the last one files it |
| `Permissions` | A key ring: one key per action (read / actuate / provide / create), each key cut for a set of paths, with an expiry date |

**Guarantees**: slicing methods can't produce invalid UTF-8; a `Matcher` or
`Permissions` value can't exist in an invalid state because construction is
the only way in.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "`s.len()` is the number of characters" | It's the number of **bytes**; use `s.chars().count()` for scalar values. |
| "`&s[7..]` is always safe after `starts_with`" | Safe *there* because the prefix is ASCII; in general byte slicing can panic (not on a char boundary). Prefer `strip_prefix`. |
| "`static RE: Regex = Regex::new(..)` works" | Statics need const initialisers → E0015. Use `LazyLock`. |
| "Builders need `&mut self`" | Consuming (`self -> Self`) and borrowing (`&mut self -> &mut Self`) are both idiomatic; pick by whether the builder is reused. |
| "Read permission must be granted explicitly" | Any of actuate / provide / create implies read. |
| "An expired token gets `PermissionDenied`" | It's `Unauthenticated` — the client should re-authenticate. |

## Smells and idioms

* **Rebuilding a regex per call.** `scope.rs:33` compiles its regex *every
  time a token is parsed* and `.unwrap()`s it (`scope.rs:60`). Tokens are
  decoded per request, so this is a real (small) cost. A `LazyLock<Regex>`
  and `.expect("...")` would be the idiom.
* **Index slicing** `header[7..]` in `grpc/server.rs` — prefer
  `header.strip_prefix("Bearer ")`, which returns `Option<&str>` and cannot panic.
* **`lazy_static!`** in `glob.rs` / `permissions.rs` — fine in existing code;
  prefer `std::sync::LazyLock` in new code.
* **`String` parameters** where `&str` would do force callers to allocate.
  Take `&str` (or `impl AsRef<str>`) unless you need to store it.
* **Returning `bool` from validation** loses the reason. For user-facing
  errors, return `Result<_, Reason>` so the message can say *which* segment
  was bad.
* **Repeated near-identical methods** (`add_read_permission`,
  `add_actuate_permission`, ... in `permissions.rs`) — a private helper
  taking the target `&mut PathMatchBuilder` would remove the duplication.

## In the databroker

| Where | What to notice |
|---|---|
| `databroker/src/glob.rs:29` | `Matcher` — validated wrapper with a private field |
| `databroker/src/glob.rs:34` | `Matcher::new` — `""` means everything; validate then translate |
| `databroker/src/glob.rs:56` | `is_match` + the `/**` retry for branch-or-signal |
| `databroker/src/glob.rs:72` | `to_glob_string` — `.` → `/`, append `/**` without wildcards |
| `databroker/src/glob.rs:88` | `to_regex_string` — the *other* engine, used for permissions |
| `databroker/src/glob.rs:146`, `:181` | `lazy_static!` regexes in `(?x)` verbose mode |
| `databroker/src/broker.rs:1270` | `is_valid_path` guards registration |
| `databroker/src/broker.rs:1292` | `glob_path` precomputed per entry |
| `databroker/src/permissions.rs:22` | `ALLOW_ALL` / `ALLOW_NONE` lazy statics |
| `databroker/src/permissions.rs:39` | `Permissions`: four `PathMatcher`s + expiry |
| `databroker/src/permissions.rs:91`–`160` | The consuming builder |
| `databroker/src/permissions.rs:167` | `can_read` — read implied by the other scopes |
| `databroker/src/permissions.rs:250`, `:277` | `PathMatchBuilder` state machine |
| `databroker/src/authorization/jwt/scope.rs:33` | Scope string → `Vec<Scope>` |
| `databroker/src/authorization/jwt/decoder.rs:134` | `TryFrom<Claims> for Permissions` |
| `doc/wildcard_matching.md` | The user-facing matching rules, with examples |
| `doc/authorization.md` | Scopes and tokens from the operator's point of view |

## Exercises

| Exercise | Goal |
|---|---|
| `c29_e01_string_methods` | `split`, `rsplit_once`, `strip_suffix`, `replace`, `join`; returning borrowed slices |
| `c29_e02_validate_vss_path` | Same rule twice: iterators and a `LazyLock<Regex>` |
| `c29_e03_glob_to_matcher` | A validated `Matcher` with the databroker's translation rules |
| `c29_e04_builder_pattern` | A consuming `PermissionBuilder` with an absorbing `Everything` state |
| `c29_e05_permissions_can_read` | Scope string → permissions → `can_read` / `can_write_*` with expiry |

## Further reading

* Rust Book ch. 8.2 "Storing UTF-8 Encoded Text with Strings".
* `str` API docs — <https://doc.rust-lang.org/std/primitive.str.html>.
* `regex` crate docs — syntax, `RegexSet`, verbose mode `(?x)`.
* `std::sync::LazyLock` / `OnceLock` docs.
* "Parse, don't validate" — Alexis King (the idea behind `Matcher`).
* Rust API Guidelines, C-BUILDER — <https://rust-lang.github.io/api-guidelines/type-safety.html#c-builder>.

## Check yourself

1. Why can `fn parent(path: &str) -> Option<&str>` avoid allocating, but `to_slash_path` can't?
2. What happens if you write `static RE: Regex = Regex::new("...").unwrap();`?
3. Why does `Matcher::is_match` try a second time without `/**`?
4. Why does `Matcher` keep its field private?
5. A token has scope `provide:Vehicle.Speed`. Can it read `Vehicle.Speed`? Set an actuator target on it? Read `Vehicle.Width`?
6. Why are `Denied` and `Expired` separate errors?

<details><summary>Answers</summary>

1. `parent` returns a sub-slice of the input; `to_slash_path` produces
   different bytes, which must live in a new buffer (`String`).
2. Compile error (E0015): non-const function call in a static initialiser.
   Use `LazyLock`.
3. In `glob-match`, `A/B/**` doesn't match `A/B` itself, but a client asking
   for `A.B` expects the signal `A.B` to match.
4. So the only way to create one is `Matcher::new`, which validates — every
   `Matcher` is known-good.
5. Yes (read is implied by provide); no (needs actuate); no (not covered).
6. They mean different things to the client: `PERMISSION_DENIED` (this token
   will never work) vs `UNAUTHENTICATED` (get a fresh token and retry).

</details>
