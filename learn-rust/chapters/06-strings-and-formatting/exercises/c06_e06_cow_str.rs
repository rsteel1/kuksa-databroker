// c06_e06_cow_str.rs
//
// CONCEPT: `Cow<str>` ("clone on write"): return the input *borrowed* when
//          nothing needs to change, and an *owned* `String` only when it does.
// GUIDE:   chapters/06-strings-and-formatting/guide.md  (section "Cow: borrow when you can, own when you must")
// DATABROKER: databroker/src/glob.rs — `Matcher::to_glob_string` turns a VSS
//             pattern into a glob-match pattern ("Vehicle.Speed" ->
//             "Vehicle/Speed/**"). It always allocates; here you'll write a
//             version that allocates only when it has to.
//             databroker/src/viss/server.rs also uses `Cow::from(...)` for
//             WebSocket close reasons.
// EXPECT:  tests fail (the functions allocate even when nothing changes)
//
// `Cow<'a, str>` is an enum: `Cow::Borrowed(&'a str)` or `Cow::Owned(String)`.
// Both deref to `&str`, so callers use it like any string. The `'_` in
// `Cow<'_, str>` means "may borrow from the input" (lifetimes: chapter 11).
//
// YOUR TASK
//   Rewrite both functions so they return `Cow::Borrowed(input)` in the
//   no-change case and `Cow::Owned(...)` only when the text must change.
//   Rules:
//   `normalize_path`: trim surrounding whitespace, then drop ONE trailing '.'
//     (" Vehicle.Speed. " -> "Vehicle.Speed"). Trimming a borrowed &str is
//     free (it's a sub-slice), so the result can still be Borrowed!
//   `to_glob_pattern` (same rules as glob.rs `to_glob_string`):
//     replace every '.' with '/', and if the input has no '*' append "/**".
//     "*" and "**" contain neither '.' nor need a suffix -> Borrowed.
//   Do not edit the tests.

// I AM NOT DONE

use std::borrow::Cow;

fn normalize_path(input: &str) -> Cow<'_, str> {
    let trimmed = input.trim();
    let without_dot = trimmed.strip_suffix('.').unwrap_or(trimmed);
    Cow::Owned(without_dot.to_string())
}

fn to_glob_pattern(input: &str) -> Cow<'_, str> {
    let mut out = input.replace('.', "/");
    if !input.contains('*') {
        out.push_str("/**");
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_borrowed(c: Cow<'_, str>) -> bool {
        matches!(c, Cow::Borrowed(_))
    }

    #[test]
    fn normalize_values() {
        assert_eq!(normalize_path(" Vehicle.Speed. "), "Vehicle.Speed");
        assert_eq!(normalize_path("Vehicle.Speed"), "Vehicle.Speed");
        assert_eq!(normalize_path("Vehicle.."), "Vehicle.");
    }

    #[test]
    fn normalize_never_allocates() {
        assert!(is_borrowed(normalize_path("Vehicle.Speed")));
        assert!(is_borrowed(normalize_path("  Vehicle.Speed.  ")));
    }

    #[test]
    fn glob_values_match_the_databroker() {
        assert_eq!(to_glob_pattern("Vehicle.Speed"), "Vehicle/Speed/**");
        assert_eq!(to_glob_pattern("Vehicle.*.IsOpen"), "Vehicle/*/IsOpen");
        assert_eq!(to_glob_pattern("Vehicle"), "Vehicle/**");
        assert_eq!(to_glob_pattern("**"), "**");
    }

    #[test]
    fn glob_allocates_only_when_needed() {
        assert!(is_borrowed(to_glob_pattern("**")));
        assert!(is_borrowed(to_glob_pattern("*")));
        assert!(!is_borrowed(to_glob_pattern("Vehicle.Speed")));
    }
}
