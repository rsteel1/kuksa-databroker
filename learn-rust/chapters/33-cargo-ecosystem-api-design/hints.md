# Chapter 33 hints

## c33_e01_semver_and_breaking_changes
Start with `parse`: `s.split('.')` into a `Vec<&str>`, then a slice pattern
`let [a, b, c] = parts[..] else { ... }` gives you "exactly three" for free.
`str::parse::<u64>()` plus `map_err` turns each part into a number.

`bump`: `match (self.major, bump)`. The `0.x` rows come first because they are
the special case. Remember a bump resets everything to its right to 0.

`required_bump`: ask "could any caller's code stop compiling (or change
meaning)?" If yes → Major. Adding a public field breaks struct literals
(`Datapoint { ts, value }` no longer lists every field) and exhaustive
destructuring, unless the struct already had a private field. Adding a variant
breaks exhaustive `match`es, unless the enum is `#[non_exhaustive]`.

`next_version`: `changes.iter().map(...).max()` gives `Option<Bump>` (the
derived `Ord` orders Patch < Minor < Major). `map_or(current, |b| current.bump(b))`.

`caret_matches`: reject `candidate < req` first. Then the left-most non-zero
digit of `req` must be equal in `candidate`; if major and minor are both 0,
only an exact match counts.

## c33_e02_feature_flags_design
This is a graph walk. Keep a worklist (`Vec<String>`) of features to visit,
and use `resolved.features.insert(..)` returning `false` as your "already
visited" check. That also makes cycles terminate.

For each entry of a feature, check the forms in this order: `dep:`
(`strip_prefix`), weak `?/` (`split_once("?/")`), strong `/`
(`split_once('/')`), otherwise another feature (push it on the worklist).
Look the feature up with `.get()` and turn `None` into `FeatureError::Unknown`
with `ok_or_else` and `?`.

Weak entries can't be decided while walking, because the dependency might be
enabled by a feature you haven't visited yet. Collect them, and after the
loop add `dep/feat` only if the dep is non-optional or ended up in
`resolved.optional_deps`.

`unify` is one line: `requests.iter().flatten().cloned().collect()`.

## c33_e03_api_guidelines
Read the tests top to bottom and write down each decision:
`Store::new()` and `Store::default()` (derive `Default`); `value(&str) ->
Option<f64>`; `set(impl Into<String>, f64) -> Result<Option<f64>, SetError>`
(previous value, like `HashMap::insert`); `paths()` returning an iterator of
`&str`; `len`/`is_empty` (clippy wants both); `as_map`/`to_sorted_lines`/
`into_sorted_pairs`; a `StoreBuilder` with `capacity`, `read_only_prefix`,
`build`; `SetError` with `Display` + `Error`; `Debug` on everything.

`paths` can return `impl Iterator<Item = &str> + '_` built from
`self.values.keys().map(String::as_str)`.

`into_sorted_pairs(self)` takes `self` by value, which is what lets it
`into_iter()` the map and move the Strings out without cloning.

The builder: each method is `fn x(mut self, ...) -> Self { ...; self }`. Put
`#[must_use]` on the type so a forgotten `.build()` warns. `ReadOnly` carries
the path so the error message can name it.

## c33_e04_editions_and_msrv
Step 1: `rustc --explain E0782`. Since edition 2021 a trait used as a type
must be written `dyn Trait`: `Box<dyn Provider>`.

Step 2: in edition 2021, `paths.into_iter()` on an array yields `String`s by
value, so `.collect()` is all you need.

Step 3: sources are the Edition Guide (one chapter per edition, listing every
change) and the release announcements on blog.rust-lang.org. When a test says
an answer is wrong, it names the item, so you only re-check that one.
Two traps: `TryFrom` was *stabilised* long before it joined the *prelude*, and
the feature resolver "2" arrived with an edition, not a release note.

Step 4 is the same `max()` pattern as c33_e01's `next_version`, with
`unwrap_or(RustVersion(1, 0))`.

## c33_e05_choosing_crates
`parse_line`: find where the text starts with
`line.find(|c: char| c.is_ascii_alphanumeric())`. The prefix before it is made
of box-drawing characters that are 3 bytes each, so count with
`.chars().count()`, then divide by 4.

Then `split_whitespace()`: the first word is the name, the second is the
version (`strip_prefix('v')`), and any remaining words are `(*)`,
`(proc-macro)` or a `(path)`.

The queries are one iterator chain each (`filter` + `map` + `collect`).
`local_crates` can use `filter_map` with `?` inside the closure:
`Some((n.name.clone(), n.local_path.clone()?))`.

`duplicates`: roots are the depth-0 lines; `entry(name).or_default().push(version)`.

`direct_dependents`: find the root's position, then walk the following lines
with `take_while(|n| n.depth > 0)` (the next root ends the block) and keep
depth 1.

`first_chain`: output is depth-first, so the first child of any line is simply
the next line if it's exactly one level deeper. Walk forward while
`depth == expected` and increase `expected` each time.

## c33_e06_review_semver_and_api_naming
Three independent bugs, one per test.

Bug 1: `AddPublicFieldToStructWithOnlyPublicFields => Bump::Minor` is wrong.
Compare it with plain `AddPublicItem`: adding a new function or type never
breaks existing code, but adding a field to a struct every one of whose
fields is already public means every `Struct { a, b }` literal callers
wrote now fails to compile (it's missing the new field) — that's Major.
The rule flips once the struct already has a private field, because then
nobody outside the crate could write that literal in the first place.

Bug 2: `(self.0 as i32) > (other.0 as i32)`. `self.0` is a `u32`; `as i32`
doesn't scale the value, it reinterprets the same 32 bits as signed, so any
packed version whose bits look negative as `i32` (roughly major ≥ 2147)
compares as *smaller* than it should. There was never a sign to fix here —
drop both casts and compare `self.0 > other.0` as the `u32`s they are.

Bug 3: `format!("{}.{}", self.major(), self.minor())` only has two
placeholders; the doc comment above it promises `MAJOR.MINOR.PATCH`. Add
the third placeholder and `self.patch()`.
