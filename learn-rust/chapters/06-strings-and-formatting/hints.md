# Chapter 06 hints

## c06_e01_utf8_bytes_chars
Run `rustc --explain E0277`. Rust refuses `s[0]` on purpose: in UTF-8 a
character can be 1–4 bytes, so "the character at index 0" isn't a cheap,
well-defined operation.

`s.chars()` gives you an iterator of `char`s. What does `.next()` on it return
for an empty string? For the length, count the chars instead of the bytes.

For `truncate_chars`: slicing `&s[..n]` takes a *byte* offset and panics if it
lands inside a character. `s.char_indices()` yields `(byte_offset, char)`
pairs. The byte offset of the character *after* the ones you keep is exactly
where to cut: `s.char_indices().nth(max)`. If there is no such character,
the whole string fits.

## c06_e02_string_building
`branch + "." + leaf` calls `Add::add(branch, ".")`: `branch` is moved into
the `+` and its buffer is reused. The error (E0382) is the later use of `branch`.

`format!("{branch}.{leaf}")` only *borrows* `branch`, so it stays usable.

`join_path`: slices of `&str` have a `join` method.

`render_table`: `let mut out = String::with_capacity(...)`, then in a `for`
loop `writeln!(out, "{path}={value:.1}")`. `writeln!` returns a
`fmt::Result`. Writing into a `String` can't fail, so
`.expect("writing to a String cannot fail")` is honest here.

## c06_e03_parse_fromstr
`parse_percent`: `s.trim().parse::<u8>()` already rejects "-1" and "abc"
(they aren't valid `u8`s). `.ok()` turns the `Result` into an `Option`. What
is left is the "at most 100" rule, and a `match` with a guard
(`Some(n) if n <= 100`) reads nicely.

`from_str`: `s.split_once('=')` returns `Option<(&str, &str)>`, where `None`
means there's no `=`. Trim both parts and check the path. Then call
`value.parse::<f64>()` and `match` on `Ok(number)` / `Err(_)`. The
`BadNumber` payload is the *trimmed* value text, as an owned `String`.

## c06_e04_formatting_traits
`{:?}` needs `Debug`, which you can derive: put `#[derive(Debug)]` above the struct.

`Reading`'s Display: `write!(f, "{} = {:.2}", ...)` returns an
`fmt::Result`, which is exactly what `fmt` must return. Use a `match` on
`&self.unit` with one `write!` per arm.

`Unit`: if you write `write!(f, "{}", self.0)`, the caller's width (`{:>6}`)
is ignored, because the inner `{}` has no width. `Formatter` has a method
that writes a `&str` *while applying* the caller's width, fill and
alignment: `f.pad(&self.0)`.

`format_row`: `{path:<20}` left-aligns in 20 columns. `{value:>8.2}` is
right-aligned, width 8, 2 decimals. For `id_hex`, use `{:#06x}`: `#` adds
`0x`, `0` pads with zeros, and `6` is the total width *including* the `0x`.

## c06_e05_advanced_patterns
Slice patterns: `[]`, `["Vehicle"]`, `["Vehicle", .., "IsOpen"]`. Order
matters: the first matching arm wins, so put specific arms first.

For "deep", bind the tail with `rest @ ..` and use a guard:
`["Vehicle", rest @ ..] if rest.len() > 3 => "deep"`.

`speed_band`: `n @ 0..=30 => format!("slow ({n})")` binds the number while
matching a range.

`is_numeric`: `matches!(value, Value::Int(_) | Value::Float(_))`.

`bump_ints`: iterate with `values.iter_mut()`. `if let Value::Int(n) = value`
gives `n: &mut i64` automatically (match ergonomics), so `*n += 1` works.

`int_target`: nest the pattern all the way down:
`Update { target: Some(Some(Value::Int(t))), .. } => Some(*t)`.

## c06_e06_cow_str
The tests don't just check the text. They also check *whether it was borrowed*.

`normalize_path`: `trim()` and `strip_suffix('.')` both return sub-slices of
the input. A sub-slice of `input` is still a `&str` borrowing `input`, so you
can return `Cow::Borrowed(...)` in every case, with no `to_string()` at all.

`to_glob_pattern`: you only need a new `String` if there is a '.' to replace
or a "/**" to append. Check both conditions first, and if neither applies,
`return Cow::Borrowed(input);`. Otherwise build the String as before and
return `Cow::Owned(out)`.

## c06_e07_build_path_formatting
No bug to fix — the tests are the spec, and you write every function.

`safe_truncate`: if `s.len() <= max_bytes` there's nothing to do. Otherwise
walk `end` down from `max_bytes` while `!s.is_char_boundary(end)`, then
slice `&s[..end]`. `is_char_boundary` is exactly the check that tells you
whether cutting there would split a character.

`format_row`: one `format!` with named width and precision:
`format!("{name:>width$}: {value:.2}")`.

`parse_signal_reading`: `input.split_once('=')` gives `Option<(&str, &str)>`;
turn the `None` case into an `Err` with `.ok_or_else(...)`. Then
`.trim().parse::<f64>()` on the right-hand side, turning its `Err` into
your own message with `.map_err(...)`. The `?` operator threads both
through.

`join_paths_or_default`: `if paths.is_empty() { Cow::Borrowed(default) }
else { Cow::Owned(paths.join(".")) }`.

## c06_e08_review_log_formatter
Compare each function's doc comment against what it actually does.

Bug 1 (`format_reading`): `{value:.0}` rounds to a whole number, throwing
away the decimals the doc comment promises. The spec calls for `{:.2}`.

Bug 2 (`join_readings_skipping_zero`): inside the zero-value branch,
`let skipped = skipped + 1;` declares a new local that shadows the outer
counter and is dropped the moment the `if` block ends — the real
`skipped` never moves. Use `skipped += 1;` instead.

Bug 3 (`last_n_words`): the slice `words[start..words.len() - 1]` always
excludes the very last word, no matter what `n` is. The upper bound
should just be `words.len()` (i.e. `words[start..]`).
