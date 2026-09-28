// c06_e07_build_path_formatting.rs
//
// CONCEPT: writing several small functions from a spec: char-boundary-safe
//          truncation, formatting traits, `parse`, and `Cow`.
// GUIDE:   chapters/06-strings-and-formatting/guide.md  (section "UTF-8: bytes, chars and why s[0] doesn't compile")
// DATABROKER: databroker/src/glob.rs — `Matcher::to_glob_string` builds
//             text field by field; databroker/src/vss.rs parses signal
//             definitions the same way `parse_signal_reading` does here.
// EXPECT:  tests fail: todo!() panics.
//
// This exercise has no missing piece for you to fix — the tests below ARE
// the spec. Implement each function so every test passes.
//
// YOUR TASK
//   Implement `safe_truncate`, `format_row`, `parse_signal_reading` and
//   `join_paths_or_default` below.
//   Do not edit the tests.

use std::borrow::Cow;

/// `s` truncated to at most `max_bytes` bytes, never splitting a
/// multi-byte character. If `s` already fits, returns it unchanged.
fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// `name` right-aligned in a field `width` characters wide, followed by
/// `": "` and `value` with exactly two decimals.
/// `format_row("Speed", 42.5, 10)` -> `"     Speed: 42.50"`.
fn format_row(name: &str, value: f64, width: usize) -> String {
    format!("{name:>width$}: {value:.2}")
}

/// Parses a `"path=value"` line. `Err` with any message describing the
/// problem if there's no `=`, or the part after it isn't a valid `f64`.
fn parse_signal_reading(input: &str) -> Result<(String, f64), String> {
    let (path, value) = input
        .split_once('=')
        .ok_or_else(|| format!("missing '=' in {input:?}"))?;
    let value: f64 = value
        .trim()
        .parse()
        .map_err(|_| format!("{value:?} is not a valid number in {input:?}"))?;
    Ok((path.to_string(), value))
}

/// `"."`-joins `paths`. If `paths` is empty, borrows `default` instead
/// of allocating anything.
fn join_paths_or_default<'a>(paths: &[String], default: &'a str) -> Cow<'a, str> {
    if paths.is_empty() {
        Cow::Borrowed(default)
    } else {
        Cow::Owned(paths.join("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_never_splits_a_char() {
        assert_eq!(safe_truncate("café", 5), "café");
        assert_eq!(safe_truncate("café", 4), "caf");
        assert_eq!(safe_truncate("hello", 3), "hel");
        assert_eq!(safe_truncate("hi", 10), "hi");
    }

    #[test]
    fn row_is_right_aligned_with_two_decimals() {
        assert_eq!(format_row("Speed", 42.5, 10), "     Speed: 42.50");
        assert_eq!(format_row("RPM", 1800.0, 3), "RPM: 1800.00");
    }

    #[test]
    fn parses_valid_reading() {
        assert_eq!(
            parse_signal_reading("Vehicle.Speed=42.5"),
            Ok(("Vehicle.Speed".to_string(), 42.5))
        );
    }

    #[test]
    fn rejects_missing_equals() {
        assert!(parse_signal_reading("no-equals-sign").is_err());
    }

    #[test]
    fn rejects_bad_number() {
        assert!(parse_signal_reading("Vehicle.Speed=nan-ish").is_err());
    }

    #[test]
    fn empty_paths_borrow_the_default() {
        let result = join_paths_or_default(&[], "Vehicle");
        assert_eq!(result, "Vehicle");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn nonempty_paths_are_joined_and_owned() {
        let paths = vec![
            "Vehicle".to_string(),
            "Cabin".to_string(),
            "Door".to_string(),
        ];
        let result = join_paths_or_default(&paths, "Vehicle");
        assert_eq!(result, "Vehicle.Cabin.Door");
        assert!(matches!(result, Cow::Owned(_)));
    }
}
